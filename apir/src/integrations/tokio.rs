use std::{
    io::Result,
    net::{Shutdown, SocketAddr},
};

use crate::traits;
use async_trait::async_trait;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_util::compat::*;

#[async_trait]
impl traits::TcpStream for Compat<TcpStream> {
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
    async fn accept(&self) -> Result<(Compat<TcpStream>, SocketAddr)> {
        let (socket, addr) = TcpListener::accept(self).await?;
        Ok((socket.compat(), addr))
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

pub struct Tokio;

#[async_trait]
impl traits::ProxyTcpStream for Tokio {
    type TcpStream = Compat<TcpStream>;

    async fn tcp_connect(&self, addr: SocketAddr) -> Result<Self::TcpStream> {
        Ok(TcpStream::connect(addr).await?.compat())
    }
}

#[async_trait]
impl traits::ProxyTcpListener for Tokio {
    type TcpStream = Compat<TcpStream>;
    type TcpListener = TcpListener;

    async fn tcp_bind(&self, addr: SocketAddr) -> Result<Self::TcpListener> {
        TcpListener::bind(addr).await
    }
}

#[async_trait]
impl traits::ProxyUdpSocket for Tokio {
    type UdpSocket = UdpSocket;

    async fn udp_bind(&self, addr: SocketAddr) -> Result<Self::UdpSocket> {
        UdpSocket::bind(addr).await
    }
}

impl traits::Spawn for Tokio {
    fn spawn<Fut>(&self, future: Fut) -> () {
        // tokio::spawn()
    }
}
