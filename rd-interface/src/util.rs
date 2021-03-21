use crate::interface::{async_trait, AsyncRead, AsyncWrite, ITcpStream, TcpStream};
use futures_util::{future::try_join, io::copy, AsyncReadExt, AsyncWriteExt};
use std::{
    collections::VecDeque,
    io,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

/// Connect two `TcpStream`
pub async fn connect_tcp(t1: TcpStream, t2: TcpStream) -> io::Result<(u64, u64)> {
    let (mut read_1, mut write_1) = t1.split();
    let (mut read_2, mut write_2) = t2.split();

    try_join(
        async {
            let r = copy(&mut read_1, &mut write_2).await;
            write_2.close().await?;
            r
        },
        async {
            let r = copy(&mut read_2, &mut write_1).await;
            write_1.close().await?;
            r
        },
    )
    .await
}

pub struct PeekableTcpStream {
    tcp: TcpStream,
    buf: VecDeque<u8>,
}

impl AsyncRead for PeekableTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let (first, ..) = &self.buf.as_slices();
        if first.len() > 0 {
            let read = first.len().min(buf.len());
            buf[0..read].copy_from_slice(&first[0..read]);

            // remove 0..read
            self.buf.drain(0..read);

            Poll::Ready(Ok(read))
        } else {
            Pin::new(&mut self.tcp).poll_read(cx, buf)
        }
    }
}
impl AsyncWrite for PeekableTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.tcp).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.tcp).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.tcp).poll_close(cx)
    }
}

#[async_trait]
impl ITcpStream for PeekableTcpStream {
    async fn peer_addr(&self) -> crate::Result<SocketAddr> {
        self.tcp.peer_addr().await
    }

    async fn local_addr(&self) -> crate::Result<SocketAddr> {
        self.tcp.local_addr().await
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
