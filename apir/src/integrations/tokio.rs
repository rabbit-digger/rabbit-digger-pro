use std::{
    future::Future,
    io::{ErrorKind, Result},
    net::{Shutdown, SocketAddr},
    time::Duration,
};

use crate::traits;
use async_trait::async_trait;
use tokio::{
    net::{TcpListener, TcpStream, UdpSocket},
    time::sleep,
};
use tokio_util::compat::*;
use traits::ProxyResolver;

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
    async fn local_addr(&self) -> Result<SocketAddr> {
        TcpListener::local_addr(&self)
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
pub struct Tokio;

#[async_trait]
impl traits::ProxyTcpStream for Tokio {
    type TcpStream = Compat<TcpStream>;

    async fn tcp_connect<A: traits::IntoAddress>(&self, addr: A) -> Result<Self::TcpStream> {
        Ok(TcpStream::connect(self.resolve(addr).await?)
            .await?
            .compat())
    }
}

#[async_trait]
impl traits::ProxyTcpListener for Tokio {
    type TcpStream = Compat<TcpStream>;
    type TcpListener = TcpListener;

    async fn tcp_bind<A: traits::IntoAddress>(&self, addr: A) -> Result<Self::TcpListener> {
        TcpListener::bind(self.resolve(addr).await?).await
    }
}

#[async_trait]
impl traits::ProxyUdpSocket for Tokio {
    type UdpSocket = UdpSocket;

    async fn udp_bind<A: traits::IntoAddress>(&self, addr: A) -> Result<Self::UdpSocket> {
        UdpSocket::bind(self.resolve(addr).await?).await
    }
}

#[async_trait]
impl traits::Runtime for Tokio {
    fn spawn<Fut>(&self, future: Fut)
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send,
    {
        tokio::spawn(future);
    }
    async fn sleep(&self, duration: Duration) {
        sleep(duration).await
    }
}

#[async_trait]
impl traits::ProxyResolver for Tokio {
    async fn resolve_domain(&self, domain: (&str, u16)) -> Result<SocketAddr> {
        self.lookup_host(domain).await
    }
}

impl Tokio {
    async fn lookup_host(&self, domain: (&str, u16)) -> Result<SocketAddr> {
        tokio::net::lookup_host(domain)
            .await?
            .next()
            .ok_or(ErrorKind::AddrNotAvailable.into())
    }
}
