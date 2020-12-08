use std::{
    io::Result,
    net::{Shutdown, SocketAddr},
};

use crate::traits::{self, ProxyRuntime};
use async_trait::async_trait;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_util::compat::*;

#[async_trait]
impl traits::TcpStream for Compat<TcpStream> {
    async fn connect(addr: SocketAddr) -> Result<Self> {
        Ok(TcpStream::connect(addr).await?.compat())
    }
    async fn peer_addr(&self) -> Result<SocketAddr> {
        self.get_ref().peer_addr()
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        self.get_ref().local_addr()
    }
    async fn shutdown(&self, how: Shutdown) -> Result<()> {
        self.get_ref().shutdown(how)
    }
}

#[async_trait]
impl traits::TcpListener<Compat<TcpStream>> for TcpListener {
    async fn bind(addr: SocketAddr) -> Result<Self> {
        TcpListener::bind(addr).await
    }
    async fn accept(&self) -> Result<(Compat<TcpStream>, SocketAddr)> {
        let (socket, addr) = TcpListener::accept(self).await?;
        Ok((socket.compat(), addr))
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

struct Tokio;
impl ProxyRuntime for Tokio {
    type TcpListener = TcpListener;
    type TcpStream = Compat<TcpStream>;
    type UdpSocket = UdpSocket;
}
