use std::{io, net::SocketAddr};

pub use crate::Address;
pub use async_trait::async_trait;
pub use futures::future::RemoteHandle;
pub use futures::io::{AsyncRead, AsyncWrite};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("Not implemented")]
    NotImplemented,
}
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A TcpListener
#[async_trait]
pub trait TcpListener: Unpin + Send + Sync {
    async fn accept(&self) -> Result<(BoxTcpStream, SocketAddr)>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub type BoxTcpListener = Box<dyn TcpListener>;

/// A TcpStream
#[async_trait]
pub trait TcpStream: AsyncRead + AsyncWrite + Unpin + Send + Sync {
    async fn peer_addr(&self) -> Result<SocketAddr>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub type BoxTcpStream = Box<dyn TcpStream>;

/// A UdpSocket
#[async_trait]
pub trait UdpSocket: Unpin + Send + Sync {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)>;
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub type BoxUdpSocket = Box<dyn UdpSocket>;

#[async_trait]
pub trait ProxyNet: Unpin + Send + Sync {
    async fn tcp_connect(&self, addr: Address) -> Result<BoxTcpStream>;
    async fn tcp_bind(&self, addr: Address) -> Result<BoxTcpListener>;
    async fn udp_bind(&self, addr: Address) -> Result<BoxUdpSocket>;
}
pub type BoxProxyNet = Box<dyn ProxyNet>;

pub struct PluginInfo {
    pub name: String,
    pub net: BoxProxyNet,
}
