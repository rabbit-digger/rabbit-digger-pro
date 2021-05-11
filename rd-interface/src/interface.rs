use std::net::SocketAddr;

pub use crate::{pool::ConnectionPool, Context};
pub use crate::{Address, Error, Result};
pub use async_trait::async_trait;
pub use futures_util::future::RemoteHandle;
pub use std::sync::Arc;
pub use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub trait IntoDyn<DynType> {
    fn into_dyn(self) -> DynType
    where
        Self: Sized + 'static;
}

/// A TcpListener.
#[async_trait]
pub trait ITcpListener: Unpin + Send + Sync {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub type TcpListener = Box<dyn ITcpListener>;

impl<T: ITcpListener> IntoDyn<TcpListener> for T {
    fn into_dyn(self) -> TcpListener
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

/// A TcpStream.
#[async_trait]
pub trait ITcpStream: AsyncRead + AsyncWrite + Unpin + Send + Sync {
    async fn peer_addr(&self) -> Result<SocketAddr>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub type TcpStream = Box<dyn ITcpStream>;

impl<T: ITcpStream> IntoDyn<TcpStream> for T {
    fn into_dyn(self) -> TcpStream
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

/// A UdpSocket.
#[async_trait]
pub trait IUdpSocket: Unpin + Send + Sync {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)>;
    async fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub type UdpSocket = Arc<dyn IUdpSocket>;

impl<T: IUdpSocket> IntoDyn<UdpSocket> for T {
    fn into_dyn(self) -> UdpSocket
    where
        Self: Sized + 'static,
    {
        Arc::new(self)
    }
}

/// A Net.
#[async_trait]
pub trait INet: Unpin + Send + Sync {
    async fn tcp_connect(&self, ctx: &mut Context, addr: Address) -> Result<TcpStream>;
    async fn tcp_bind(&self, ctx: &mut Context, addr: Address) -> Result<TcpListener>;
    async fn udp_bind(&self, ctx: &mut Context, addr: Address) -> Result<UdpSocket>;
}
pub type Net = Arc<dyn INet>;

impl<T: INet> IntoDyn<Net> for T {
    fn into_dyn(self) -> Net
    where
        Self: Sized + 'static,
    {
        Arc::new(self)
    }
}

/// A Server.
#[async_trait]
pub trait IServer: Unpin + Send + Sync {
    async fn start(&self, pool: ConnectionPool) -> Result<()>;
}
pub type Server = Box<dyn IServer>;

impl<T: IServer> IntoDyn<Server> for T {
    fn into_dyn(self) -> Server
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}
