use std::{
    io::{self, ErrorKind},
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use rd_interface::{
    async_trait, Address, BoxTcpListener, BoxTcpStream, BoxUdpSocket, Plugin, ProxyNet, Registry,
    Result,
};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_util::compat::*;

pub struct Net;
pub struct CompatTcp(Compat<TcpStream>);
pub struct Listener(TcpListener);
pub struct Udp(UdpSocket);

impl Net {
    fn new() -> Net {
        Net
    }
}
async fn lookup_host(domain: String, port: u16) -> io::Result<SocketAddr> {
    let domain = (domain.as_ref(), port);
    tokio::net::lookup_host(domain)
        .await?
        .next()
        .ok_or(ErrorKind::AddrNotAvailable.into())
}

impl rd_interface::AsyncRead for CompatTcp {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}
impl rd_interface::AsyncWrite for CompatTcp {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

#[async_trait]
impl rd_interface::TcpStream for CompatTcp {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        self.0.get_ref().peer_addr().map_err(Into::into)
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.get_ref().local_addr().map_err(Into::into)
    }
}
impl CompatTcp {
    fn new(t: TcpStream) -> Box<CompatTcp> {
        Box::new(CompatTcp(t.compat()))
    }
}

#[async_trait]
impl rd_interface::TcpListener for Listener {
    async fn accept(&self) -> Result<(BoxTcpStream, SocketAddr)> {
        let (socket, addr) = TcpListener::accept(&self.0).await?;
        Ok((CompatTcp::new(socket), addr))
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        TcpListener::local_addr(&self.0).map_err(Into::into)
    }
}

#[async_trait]
impl rd_interface::UdpSocket for Udp {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        UdpSocket::recv_from(&self.0, buf).await.map_err(Into::into)
    }

    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        UdpSocket::send_to(&self.0, buf, addr)
            .await
            .map_err(Into::into)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        UdpSocket::local_addr(&self.0).map_err(Into::into)
    }
}

#[async_trait]
impl ProxyNet for Net {
    async fn tcp_connect(&self, addr: Address) -> Result<BoxTcpStream> {
        let addr = addr.resolve(lookup_host).await?;
        Ok(CompatTcp::new(TcpStream::connect(addr).await?))
    }

    async fn tcp_bind(&self, addr: Address) -> Result<BoxTcpListener> {
        let addr = addr.resolve(lookup_host).await?;
        Ok(Box::new(Listener(TcpListener::bind(addr).await?)))
    }

    async fn udp_bind(&self, addr: Address) -> Result<BoxUdpSocket> {
        let addr = addr.resolve(lookup_host).await?;
        Ok(Box::new(Udp(UdpSocket::bind(addr).await?)))
    }
}

#[no_mangle]
pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_plugin("local", Plugin::Net(Box::new(Net::new())));
    Ok(())
}
