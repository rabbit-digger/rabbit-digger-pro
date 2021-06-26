use ::std::{io, pin::Pin, task};
use std::net::SocketAddr;

use crate::tls::TlsStream;
use futures::ready;
use rd_interface::{
    async_trait, impl_async_read, AsyncWrite, ITcpStream, TcpStream, NOT_IMPLEMENTED,
};

pub(super) struct TrojanTcp {
    stream: TlsStream<TcpStream>,
    head: Option<Vec<u8>>,
    is_first: bool,
}

impl TrojanTcp {
    pub fn new(stream: TlsStream<TcpStream>, head: Vec<u8>) -> Self {
        Self {
            stream,
            head: Some(head),
            is_first: true,
        }
    }
}

impl_async_read!(TrojanTcp, stream);

impl AsyncWrite for TrojanTcp {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> task::Poll<io::Result<usize>> {
        loop {
            let Self {
                stream,
                head,
                is_first,
            } = &mut *self;
            let stream = Pin::new(stream);
            let len = match head {
                Some(head) => {
                    if *is_first {
                        head.extend(buf);
                        *is_first = false;
                    }

                    let sent = ready!(stream.poll_write(cx, &head))?;
                    head.drain(..sent);
                    head.len()
                }
                None => break,
            };
            if len == 0 {
                *head = None;
                return task::Poll::Ready(Ok(buf.len()));
            }
        }

        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> task::Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.stream).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }
}

#[async_trait]
impl ITcpStream for TrojanTcp {
    async fn peer_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    async fn local_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}
