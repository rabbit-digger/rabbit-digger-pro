use rd_interface::{async_trait, impl_async_write, AsyncRead, ITcpStream, TcpStream};
use std::{
    collections::VecDeque,
    io,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
};
use tokio::io::{AsyncReadExt, ReadBuf};

pub struct PeekableTcpStream {
    tcp: TcpStream,
    buf: VecDeque<u8>,
}

#[async_trait]
impl ITcpStream for PeekableTcpStream {
    async fn peer_addr(&self) -> crate::Result<SocketAddr> {
        self.tcp.peer_addr().await
    }

    async fn local_addr(&self) -> crate::Result<SocketAddr> {
        self.tcp.local_addr().await
    }

    impl_async_write!(tcp);

    fn poll_read(&mut self, cx: &mut task::Context<'_>, buf: &mut ReadBuf) -> Poll<io::Result<()>> {
        let (first, ..) = &self.buf.as_slices();
        if !first.is_empty() {
            let read = first.len().min(buf.remaining());
            let unfilled = buf.initialize_unfilled_to(read);
            unfilled[0..read].copy_from_slice(&first[0..read]);
            buf.advance(read);

            // remove 0..read
            self.buf.drain(0..read);

            Poll::Ready(Ok(()))
        } else {
            Pin::new(&mut self.tcp).poll_read(cx, buf)
        }
    }
}

impl PeekableTcpStream {
    pub fn new(tcp: TcpStream) -> Self {
        PeekableTcpStream {
            tcp,
            buf: VecDeque::new(),
        }
    }
    // Fill self.buf to size using self.tcp.read_exact
    async fn fill_buf(&mut self, size: usize) -> crate::Result<()> {
        if size > self.buf.len() {
            let to_read = size - self.buf.len();
            let mut buf = vec![0u8; to_read];
            self.tcp.read_exact(&mut buf).await?;
            self.buf.append(&mut buf.into());
        }
        Ok(())
    }
    pub async fn peek_exact(&mut self, buf: &mut [u8]) -> crate::Result<()> {
        self.fill_buf(buf.len()).await?;
        let self_buf = self.buf.make_contiguous();
        buf.copy_from_slice(&self_buf[0..buf.len()]);

        Ok(())
    }
    pub fn into_inner(self) -> (TcpStream, VecDeque<u8>) {
        (self.tcp, self.buf)
    }
}
