use std::net::SocketAddr;

use crate::Context;
pub use crate::{Address, Error, Result};
pub use async_trait::async_trait;
pub use futures_io::{AsyncRead, AsyncWrite};
pub use futures_util::future::RemoteHandle;
pub use std::sync::Arc;

/// A TcpListener
#[async_trait]
pub trait ITcpListener: Unpin + Send + Sync {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub type TcpListener = Box<dyn ITcpListener>;

/// A TcpStream
#[async_trait]
pub trait ITcpStream: AsyncRead + AsyncWrite + Unpin + Send + Sync {
    async fn peer_addr(&self) -> Result<SocketAddr>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub type TcpStream = Box<dyn ITcpStream>;

/// A UdpSocket
#[async_trait]
pub trait IUdpSocket: Unpin + Send + Sync {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)>;
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub type UdpSocket = Box<dyn IUdpSocket>;

#[async_trait]
pub trait INet: Unpin + Send + Sync {
    async fn tcp_connect(&self, ctx: &mut Context, addr: Address) -> Result<TcpStream>;
    async fn tcp_bind(&self, ctx: &mut Context, addr: Address) -> Result<TcpListener>;
    async fn udp_bind(&self, ctx: &mut Context, addr: Address) -> Result<UdpSocket>;
}
pub type Net = Arc<dyn INet>;

#[async_trait]
pub trait IServer: Unpin + Send + Sync {
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
}
pub type Server = Box<dyn IServer>;
