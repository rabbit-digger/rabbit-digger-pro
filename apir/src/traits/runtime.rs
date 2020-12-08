use async_trait::async_trait;
use futures::io::{AsyncRead, AsyncWrite};
use std::{
    error,
    fmt::{self, Display, Formatter},
    io::{Error, ErrorKind, Result},
    net::{Shutdown, SocketAddr},
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

/// A TcpListener
#[async_trait]
pub trait TcpListener<TcpStream>: Sized {
    async fn bind(addr: SocketAddr) -> Result<Self>;
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)>;
}

/// A TcpStream
#[async_trait]
pub trait TcpStream: AsyncRead + AsyncWrite + Sized {
    async fn connect(addr: SocketAddr) -> Result<Self>;
    async fn peer_addr(&self) -> Result<SocketAddr>;
    async fn local_addr(&self) -> Result<SocketAddr>;
    async fn shutdown(&self, how: Shutdown) -> Result<()>;
}

/// A UdpSocket
#[async_trait]
pub trait UdpSocket: Sized {
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
