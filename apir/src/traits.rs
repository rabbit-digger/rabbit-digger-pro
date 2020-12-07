//! Defines traits used in APiR.
use async_trait::async_trait;
use futures::io::{AsyncRead, AsyncWrite};
use std::{io, net::Shutdown, net::SocketAddr, net::ToSocketAddrs};

/// A TcpStream
#[async_trait]
pub trait TcpStream: AsyncRead + AsyncWrite + Sized {
    async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self>;
    async fn peer_addr(&self) -> io::Result<SocketAddr>;
    async fn local_addr(&self) -> io::Result<SocketAddr>;
    async fn shutdown(&self, how: Shutdown) -> io::Result<()>;
}

/// A UdpSocket
#[async_trait]
pub trait UdpSocket {
    async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
    async fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addr: A) -> io::Result<usize>;
    async fn peer_addr(&self) -> io::Result<SocketAddr>;
    async fn local_addr(&self) -> io::Result<SocketAddr>;
}

/// A proxy client
pub trait ProxyClient {
    type TcpStream: TcpStream;
    type UdpSocket: UdpSocket;
}
