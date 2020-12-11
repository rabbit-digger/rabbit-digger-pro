pub use async_trait::async_trait;
use futures::future::FutureExt;
pub use futures::future::RemoteHandle;
pub use futures::io::{AsyncRead, AsyncWrite};
use std::{
    future::Future,
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

#[async_trait]
impl<T: ProxyTcpStream> ProxyTcpStream for &T {
    const NOT_SUPPORT: bool = T::NOT_SUPPORT;
    type TcpStream = T::TcpStream;

    async fn tcp_connect(&self, addr: SocketAddr) -> Result<Self::TcpStream> {
        ProxyTcpStream::tcp_connect(*self, addr).await
    }
}

/// A proxy tcp listener
#[async_trait]
pub trait ProxyTcpListener: Unpin + Sized + Send + Sync {
    const NOT_SUPPORT: bool = false;
    type TcpStream: TcpStream;
    type TcpListener: TcpListener<Self::TcpStream>;
    async fn tcp_bind(&self, addr: SocketAddr) -> Result<Self::TcpListener>;
}

#[async_trait]
impl<T: ProxyTcpListener> ProxyTcpListener for &T {
    const NOT_SUPPORT: bool = T::NOT_SUPPORT;
    type TcpStream = T::TcpStream;
    type TcpListener = T::TcpListener;

    async fn tcp_bind(&self, addr: SocketAddr) -> Result<Self::TcpListener> {
        ProxyTcpListener::tcp_bind(*self, addr).await
    }
}

/// A proxy udp socket
#[async_trait]
pub trait ProxyUdpSocket: Unpin + Sized + Send + Sync {
    const NOT_SUPPORT: bool = false;
    type UdpSocket: UdpSocket;
    async fn udp_bind(&self, addr: SocketAddr) -> Result<Self::UdpSocket>;
}

#[async_trait]
impl<T: ProxyUdpSocket> ProxyUdpSocket for &T {
    const NOT_SUPPORT: bool = T::NOT_SUPPORT;
    type UdpSocket = T::UdpSocket;

    async fn udp_bind(&self, addr: SocketAddr) -> Result<Self::UdpSocket> {
        ProxyUdpSocket::udp_bind(*self, addr).await
    }
}

#[async_trait]
pub trait Spawn: Unpin + Sized + Send + Sync {
    fn spawn_handle<Fut>(&self, future: Fut) -> RemoteHandle<Fut::Output>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send,
    {
        let (future, handle) = future.remote_handle();
        self.spawn(future);
        handle
    }
    fn spawn<Fut>(&self, future: Fut)
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send;
}

impl<T: Spawn> Spawn for &T {
    fn spawn<Fut>(&self, future: Fut)
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send,
    {
        Spawn::spawn(*self, future)
    }
}

pub trait ProxyNet: ProxyTcpStream + ProxyTcpListener + ProxyUdpSocket {}
