use std::{
    future::Future,
    io::Result,
    net::{Shutdown, SocketAddr},
    time::Duration,
};

use crate::traits;
use async_std::{
    net::{TcpListener, TcpStream, UdpSocket},
    task::sleep,
};
use async_trait::async_trait;

#[async_trait]
impl traits::TcpStream for TcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        self.peer_addr()
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        self.local_addr()
    }
    async fn shutdown(&self, how: Shutdown) -> Result<()> {
        self.shutdown(how)
    }
}

#[async_trait]
impl traits::TcpListener<TcpStream> for TcpListener {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        let (socket, addr) = TcpListener::accept(self).await?;
        Ok((socket, addr))
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        self.local_addr()
    }
}

#[async_trait]
impl traits::UdpSocket for UdpSocket {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        UdpSocket::recv_from(self, buf).await
    }

    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        UdpSocket::send_to(self, buf, addr).await
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        UdpSocket::local_addr(self)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AsyncStd;

#[async_trait]
impl traits::ProxyTcpListener for AsyncStd {
    type TcpListener = TcpListener;
    type TcpStream = TcpStream;

    async fn tcp_bind(&self, addr: SocketAddr) -> Result<Self::TcpListener> {
        TcpListener::bind(addr).await
    }
}

#[async_trait]
impl traits::ProxyTcpStream for AsyncStd {
    type TcpStream = TcpStream;

    async fn tcp_connect(&self, addr: SocketAddr) -> Result<Self::TcpStream> {
        Ok(TcpStream::connect(addr).await?)
    }
}

#[async_trait]
impl traits::ProxyUdpSocket for AsyncStd {
    type UdpSocket = UdpSocket;

    async fn udp_bind(&self, addr: SocketAddr) -> Result<Self::UdpSocket> {
        UdpSocket::bind(addr).await
    }
}

#[async_trait]
impl traits::Runtime for AsyncStd {
    fn spawn<Fut>(&self, future: Fut)
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send,
    {
        async_std::task::spawn(future);
    }
    async fn sleep(&self, duration: Duration) {
        sleep(duration).await
    }
}
