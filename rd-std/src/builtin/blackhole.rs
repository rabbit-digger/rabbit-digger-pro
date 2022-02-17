use std::{future::pending, io, net::SocketAddr, pin::Pin, task::Poll};

use futures::{Sink, Stream};
use rd_interface::{
    async_trait, config::EmptyConfig, registry::Builder, Address, AsyncRead, AsyncWrite, Bytes,
    BytesMut, INet, ITcpListener, ITcpStream, IUdpSocket, IntoDyn, Net, ReadBuf, Result,
    NOT_IMPLEMENTED,
};
pub struct BlackholeNet;

impl Builder<Net> for BlackholeNet {
    const NAME: &'static str = "blackhole";
    type Config = EmptyConfig;
    type Item = BlackholeNet;

    fn build(_config: Self::Config) -> Result<Self::Item> {
        Ok(BlackholeNet)
    }
}

struct BlackItem;

impl AsyncRead for BlackItem {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Pending
    }
}

impl AsyncWrite for BlackItem {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Poll::Pending
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Pending
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Pending
    }
}

#[async_trait]
impl ITcpStream for BlackItem {
    async fn peer_addr(&self) -> Result<std::net::SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    async fn local_addr(&self) -> Result<std::net::SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}

#[async_trait]
impl ITcpListener for BlackItem {
    async fn accept(&self) -> Result<(rd_interface::TcpStream, std::net::SocketAddr)> {
        pending().await
    }

    async fn local_addr(&self) -> Result<std::net::SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}

impl Stream for BlackItem {
    type Item = io::Result<(BytesMut, SocketAddr)>;

    fn poll_next(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        Poll::Pending
    }
}

impl Sink<(Bytes, Address)> for BlackItem {
    type Error = io::Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Pending
    }

    fn start_send(self: Pin<&mut Self>, _item: (Bytes, Address)) -> Result<(), Self::Error> {
        Ok(())
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Pending
    }

    fn poll_close(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Pending
    }
}

#[async_trait]
impl IUdpSocket for BlackItem {
    async fn local_addr(&self) -> Result<std::net::SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}

#[async_trait]
impl INet for BlackholeNet {
    async fn tcp_connect(
        &self,
        _ctx: &mut rd_interface::Context,
        _addr: &rd_interface::Address,
    ) -> Result<rd_interface::TcpStream> {
        Ok(BlackItem.into_dyn())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        _addr: &rd_interface::Address,
    ) -> Result<rd_interface::TcpListener> {
        Ok(BlackItem.into_dyn())
    }

    async fn udp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        _addr: &rd_interface::Address,
    ) -> Result<rd_interface::UdpSocket> {
        Ok(BlackItem.into_dyn())
    }

    async fn lookup_host(
        &self,
        _addr: &rd_interface::Address,
    ) -> Result<Vec<std::net::SocketAddr>> {
        Err(rd_interface::NOT_IMPLEMENTED)
    }
}
