pub use async_trait::async_trait;
pub use futures::io::{AsyncRead, AsyncWrite};
use futures::Future;
use std::{
    io::Result,
    net::{Shutdown, SocketAddr},
};

/// A TcpListener
#[async_trait]
pub trait TcpListener<TcpStream>: Unpin + Sized + Send + Sync {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)>;
}

/// A TcpStream
#[async_trait]
pub trait TcpStream: AsyncRead + AsyncWrite + Unpin + Sized + Send + Sync {
    async fn peer_addr(&self) -> Result<SocketAddr>;
    async fn local_addr(&self) -> Result<SocketAddr>;
    async fn shutdown(&self, how: Shutdown) -> Result<()>;
}

/// A UdpSocket
#[async_trait]
pub trait UdpSocket: Unpin + Sized + Send + Sync {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)>;
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}

/// A proxy tcp stream
#[async_trait]
pub trait ProxyTcpStream: Unpin + Sized + Send + Sync {
    const NOT_SUPPORT: bool = false;
    type TcpStream: TcpStream;
    async fn tcp_connect(&self, addr: SocketAddr) -> Result<Self::TcpStream>;
}

/// A proxy tcp listener
#[async_trait]
pub trait ProxyTcpListener: Unpin + Sized + Send + Sync {
    const NOT_SUPPORT: bool = false;
    type TcpStream: TcpStream;
    type TcpListener: TcpListener<Self::TcpStream>;
    async fn tcp_bind(&self, addr: SocketAddr) -> Result<Self::TcpListener>;
}

/// A proxy udp socket
#[async_trait]
pub trait ProxyUdpSocket: Unpin + Sized + Send + Sync {
    const NOT_SUPPORT: bool = false;
    type UdpSocket: UdpSocket;
    async fn udp_bind(&self, addr: SocketAddr) -> Result<Self::UdpSocket>;
}

#[async_trait]
pub trait Spawn: Unpin + Sized + Send + Sync {
    fn run(&self) {}
    fn spawn<Fut: Future>(&self, future: Fut) -> ();
}

impl<T: Spawn> Spawn for &T {
    fn spawn<Fut: Future>(&self, future: Fut) -> () {
        Spawn::spawn(*self, future)
    }
}

pub trait ProxyNet: ProxyTcpStream + ProxyTcpListener + ProxyUdpSocket {}
