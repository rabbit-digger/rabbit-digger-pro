use crate::{
    interface::{
        async_trait, AsyncRead, AsyncWrite, INet, ITcpStream, Net, TcpListener, TcpStream,
        UdpChannel, UdpSocket,
    },
    Address, Context, Result, NOT_IMPLEMENTED,
};
use futures_util::future::try_join;
use std::{
    collections::VecDeque,
    future::Future,
    io,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
};
pub use tokio::io::copy_bidirectional;
use tokio::io::{AsyncReadExt, ReadBuf};
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Connect two `TcpStream`
pub async fn connect_tcp(
    t1: impl AsyncRead + AsyncWrite,
    t2: impl AsyncRead + AsyncWrite,
) -> io::Result<()> {
    tokio::pin!(t1);
    tokio::pin!(t2);
    copy_bidirectional(&mut t1, &mut t2).await?;
    Ok(())
}

pub struct PeekableTcpStream {
    tcp: TcpStream,
    buf: VecDeque<u8>,
}

impl AsyncRead for PeekableTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        let (first, ..) = &self.buf.as_slices();
        if first.len() > 0 {
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
impl AsyncWrite for PeekableTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.tcp).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.tcp).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.tcp).poll_shutdown(cx)
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

/// A no-op Net returns [`Error::NotImplemented`](crate::Error::NotImplemented) for every method.
pub struct NotImplementedNet;

#[async_trait]
impl INet for NotImplementedNet {
    async fn tcp_connect(&self, _ctx: &mut Context, _addr: Address) -> Result<TcpStream> {
        Err(NOT_IMPLEMENTED)
    }

    async fn tcp_bind(&self, _ctx: &mut Context, _addr: Address) -> Result<TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(&self, _ctx: &mut Context, _addr: Address) -> Result<UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
}

/// A new Net calls [`tcp_connect()`](crate::INet::tcp_connect()), [`tcp_bind()`](crate::INet::tcp_bind()), [`udp_bind()`](crate::INet::udp_bind()) from different Net.
pub struct CombineNet {
    pub tcp_connect: Net,
    pub tcp_bind: Net,
    pub udp_bind: Net,
}

impl INet for CombineNet {
    #[inline(always)]
    fn tcp_connect<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<TcpStream>>
    where
        Self: 'a,
    {
        self.tcp_connect.tcp_connect(ctx, addr)
    }

    #[inline(always)]
    fn tcp_bind<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<TcpListener>>
    where
        Self: 'a,
    {
        self.tcp_bind.tcp_bind(ctx, addr)
    }

    #[inline(always)]
    fn udp_bind<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<UdpSocket>>
    where
        Self: 'a,
    {
        self.udp_bind.udp_bind(ctx, addr)
    }
}

pub async fn connect_udp(udp_channel: UdpChannel, udp: UdpSocket) -> crate::Result<()> {
    let in_side = async {
        let mut buf = [0u8; crate::constant::UDP_BUFFER_SIZE];
        while let Ok((size, addr)) = udp_channel.recv_send_to(&mut buf).await {
            let buf = &buf[..size];
            udp.send_to(buf, addr).await?;
        }
        crate::Result::<()>::Ok(())
    };
    let out_side = async {
        let mut buf = [0u8; crate::constant::UDP_BUFFER_SIZE];
        while let Ok((size, addr)) = udp.recv_from(&mut buf).await {
            let buf = &buf[..size];
            udp_channel.send_recv_from(buf, addr).await?;
        }
        crate::Result::<()>::Ok(())
    };
    try_join(in_side, out_side).await?;
    Ok(())
}
