use ::std::{io, pin::Pin, task};
use std::net::SocketAddr;

use futures::ready;
use rd_interface::{
    async_trait, impl_async_read, AsyncWrite, ITcpStream, TcpStream, NOT_IMPLEMENTED,
};
use tokio_rustls::client::TlsStream;

pub(super) struct TrojanTcp {
    pub stream: TlsStream<TcpStream>,
    pub head: Option<Vec<u8>>,
    pub is_first: bool,
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