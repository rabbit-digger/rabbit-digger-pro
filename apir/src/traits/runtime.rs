use async_trait::async_trait;
use futures::io::{AsyncRead, AsyncWrite};
use std::{
    error,
    fmt::{self, Display, Formatter},
    io::{Error, ErrorKind, Result},
    marker::PhantomData,
    net::{Shutdown, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
};

#[derive(Debug)]
pub struct NotSupport;
impl Display for NotSupport {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Not support")
    }
}
impl error::Error for NotSupport {}

pub fn not_support() -> Error {
    Error::new(ErrorKind::Other, NotSupport)
}

pub fn is_not_suppoer(err: Error) -> bool {
    let err = err.into_inner();
    err.map(|i| i.is::<NotSupport>()).unwrap_or(false)
}

pub struct NotImplement<T: Send = ()>(PhantomData<T>);
#[async_trait]
impl<T> TcpListener<T> for NotImplement<T>
where
    T: Send + Sync,
{
    const NOT_SUPPORT: bool = true;
    async fn bind(_addr: SocketAddr) -> Result<Self> {
        todo!()
    }

    async fn accept(&self) -> Result<(T, SocketAddr)> {
        todo!()
    }
}

impl AsyncRead for NotImplement {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        todo!()
    }
}
impl AsyncWrite for NotImplement {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context<'_>, _buf: &[u8]) -> Poll<Result<usize>> {
        todo!()
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        todo!()
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        todo!()
    }
}
#[async_trait]
impl TcpStream for NotImplement {
    const NOT_SUPPORT: bool = true;
    async fn connect(_addr: SocketAddr) -> Result<Self> {
        todo!()
    }
    async fn peer_addr(&self) -> Result<SocketAddr> {
        todo!()
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        todo!()
    }
    async fn shutdown(&self, _how: Shutdown) -> Result<()> {
        todo!()
    }
}

#[async_trait]
impl UdpSocket for NotImplement {
    const NOT_SUPPORT: bool = true;
    async fn bind(_addr: SocketAddr) -> Result<Self> {
        todo!()
    }
    async fn recv_from(&self, _buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        todo!()
    }
    async fn send_to(&self, _buf: &[u8], _addr: SocketAddr) -> Result<usize> {
        todo!()
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        todo!()
    }
}

/// A TcpListener
#[async_trait]
pub trait TcpListener<TcpStream>: Sized {
    const NOT_SUPPORT: bool = false;
    async fn bind(addr: SocketAddr) -> Result<Self>;
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)>;
}

/// A TcpStream
#[async_trait]
pub trait TcpStream: AsyncRead + AsyncWrite + Sized {
    const NOT_SUPPORT: bool = false;
    async fn connect(addr: SocketAddr) -> Result<Self>;
    async fn peer_addr(&self) -> Result<SocketAddr>;
    async fn local_addr(&self) -> Result<SocketAddr>;
    async fn shutdown(&self, how: Shutdown) -> Result<()>;
}

/// A UdpSocket
#[async_trait]
pub trait UdpSocket: Sized {
    const NOT_SUPPORT: bool = false;
    async fn bind(addr: SocketAddr) -> Result<Self>;
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)>;
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}

/// A proxy runtime
#[async_trait]
pub trait ProxyRuntime {
    type TcpListener: TcpListener<Self::TcpStream>;
    type TcpStream: TcpStream;
    type UdpSocket: UdpSocket;
}
