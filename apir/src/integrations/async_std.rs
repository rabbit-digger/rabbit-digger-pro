use std::{
    io::Result,
    net::{Shutdown, SocketAddr},
};

use crate::traits::{self, ProxyRuntime};
use async_trait::async_trait;
use async_std::net::{TcpStream, UdpSocket, TcpListener};

#[async_trait]
impl traits::TcpStream for TcpStream {
    async fn connect(addr: SocketAddr) -> Result<Self> {
        Ok(TcpStream::connect(addr).await?)
    }
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
    async fn bind(addr: SocketAddr) -> Result<Self> {
        TcpListener::bind(addr).await
    }

    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        let (socket, addr) = TcpListener::accept(self).await?;
        Ok((socket, addr))
    }
}

#[async_trait]
impl traits::UdpSocket for UdpSocket {
    async fn bind(addr: SocketAddr) -> Result<Self> {
        UdpSocket::bind(addr).await
    }

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
impl ProxyRuntime for AsyncStd {
    type TcpListener = TcpListener;
    type TcpStream = TcpStream;
    type UdpSocket = UdpSocket;
}
