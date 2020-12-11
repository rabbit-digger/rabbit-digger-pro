use std::{
    io::Result,
    marker::PhantomData,
    net::{Shutdown, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
};

use super::runtime::*;

pub struct NotImplement<T: Send = ()>(PhantomData<T>);
#[async_trait]
impl<T> TcpListener<T> for NotImplement<T>
where
    T: Unpin + Send + Sync,
{
    async fn accept(&self) -> Result<(T, SocketAddr)> {
        unimplemented!()
    }
}

impl AsyncRead for NotImplement {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        unimplemented!()
    }
}
impl AsyncWrite for NotImplement {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context<'_>, _buf: &[u8]) -> Poll<Result<usize>> {
        unimplemented!()
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        unimplemented!()
    }
    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        unimplemented!()
    }
}
#[async_trait]
impl TcpStream for NotImplement {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        unimplemented!()
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        unimplemented!()
    }
    async fn shutdown(&self, _how: Shutdown) -> Result<()> {
        unimplemented!()
    }
}

#[async_trait]
impl UdpSocket for NotImplement {
    async fn recv_from(&self, _buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        unimplemented!()
    }
    async fn send_to(&self, _buf: &[u8], _addr: SocketAddr) -> Result<usize> {
        unimplemented!()
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        unimplemented!()
    }
}

#[async_trait]
impl ProxyTcpListener for NotImplement {
    type TcpStream = NotImplement;
    type TcpListener = NotImplement<Self::TcpStream>;
    async fn tcp_bind(&self, _addr: SocketAddr) -> Result<Self::TcpListener> {
        unimplemented!()
    }
}

#[async_trait]
impl ProxyTcpStream for NotImplement {
    type TcpStream = NotImplement;
    async fn tcp_connect(&self, _addr: SocketAddr) -> Result<Self::TcpStream> {
        unimplemented!()
    }
}

#[async_trait]
impl ProxyUdpSocket for NotImplement {
    type UdpSocket = NotImplement;
    async fn udp_bind(&self, _addr: SocketAddr) -> Result<Self::UdpSocket> {
        unimplemented!()
    }
}
