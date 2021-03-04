use std::{
    io::{self, ErrorKind},
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use async_std::net;
use rd_interface::{
    async_trait, Address, INet, Registry, Result, TcpListener, TcpStream, UdpSocket,
};

pub struct Net;
pub struct CompatTcp(net::TcpStream);
pub struct Listener(net::TcpListener);
pub struct Udp(net::UdpSocket);

impl Net {
    fn new() -> Net {
        Net
    }
}
async fn lookup_host(domain: String, port: u16) -> io::Result<SocketAddr> {
    use async_std::net::ToSocketAddrs;

    let domain = (domain.as_ref(), port);
    ToSocketAddrs::to_socket_addrs(&domain)
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
impl rd_interface::ITcpStream for CompatTcp {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        self.0.peer_addr().map_err(Into::into)
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().map_err(Into::into)
    }
}
impl CompatTcp {
    fn new(t: net::TcpStream) -> Box<CompatTcp> {
        Box::new(CompatTcp(t))
    }
}

#[async_trait]
impl rd_interface::ITcpListener for Listener {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        let (socket, addr) = self.0.accept().await?;
        Ok((CompatTcp::new(socket), addr))
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().map_err(Into::into)
    }
}

#[async_trait]
impl rd_interface::IUdpSocket for Udp {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        self.0.recv_from(buf).await.map_err(Into::into)
    }

    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        self.0.send_to(buf, addr).await.map_err(Into::into)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().map_err(Into::into)
    }
}

#[async_trait]
impl INet for Net {
    async fn tcp_connect(&self, _ctx: &rd_interface::Context, addr: Address) -> Result<TcpStream> {
        let addr = addr.resolve(lookup_host).await?;
        Ok(CompatTcp::new(net::TcpStream::connect(addr).await?))
    }

    async fn tcp_bind(&self, _ctx: &rd_interface::Context, addr: Address) -> Result<TcpListener> {
        let addr = addr.resolve(lookup_host).await?;
        Ok(Box::new(Listener(net::TcpListener::bind(addr).await?)))
    }

    async fn udp_bind(&self, _ctx: &rd_interface::Context, addr: Address) -> Result<UdpSocket> {
        let addr = addr.resolve(lookup_host).await?;
        Ok(Box::new(Udp(net::UdpSocket::bind(addr).await?)))
    }
}

#[no_mangle]
pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_net("local", |_, _| Ok(Net::new()));
    Ok(())
}
