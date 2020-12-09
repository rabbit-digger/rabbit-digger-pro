use std::{
    io::Result,
    net::{Shutdown, SocketAddr},
};

use crate::traits::{self, ProxyRuntime};
use async_std::net::{TcpListener, TcpStream, UdpSocket};
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

struct AsyncStd;
#[async_trait]
impl ProxyRuntime for AsyncStd {
    type TcpListener = TcpListener;
    type TcpStream = TcpStream;
    type UdpSocket = UdpSocket;

    async fn tcp_connect(&self, addr: SocketAddr) -> Result<Self::TcpStream> {
        Ok(TcpStream::connect(addr).await?)
    }

    async fn tcp_bind(&self, addr: SocketAddr) -> Result<Self::TcpListener> {
        TcpListener::bind(addr).await
    }

    async fn udp_bind(&self, addr: SocketAddr) -> Result<Self::UdpSocket> {
        UdpSocket::bind(addr).await
    }
}
