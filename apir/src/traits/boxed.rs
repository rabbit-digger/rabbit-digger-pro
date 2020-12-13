use std::{io::Result, net::SocketAddr};

use super::runtime::*;

pub type BoxedTcpStream = Box<dyn TcpStream>;
pub type BoxedTcpListener<TcpStream> = Box<dyn TcpListener<TcpStream>>;
pub type BoxedUdpSocket = Box<dyn UdpSocket>;
pub struct Boxed<T: ?Sized>(T);

#[async_trait]
impl<T> TcpListener<BoxedTcpStream> for BoxedTcpListener<T>
where
    T: TcpStream + 'static,
{
    #[inline(always)]
    async fn accept(&self) -> Result<(BoxedTcpStream, SocketAddr)> {
        let (s, addr) = self.accept().await?;
        Ok(((Box::new(s)), addr))
    }
    #[inline(always)]
    async fn local_addr(&self) -> Result<SocketAddr> {
        self.local_addr().await
    }
}

#[async_trait]
impl<T> ProxyTcpStream for Boxed<T>
where
    T: ProxyTcpStream + 'static,
    T::TcpStream: 'static,
{
    type TcpStream = BoxedTcpStream;

    #[inline(always)]
    async fn tcp_connect(&self, addr: SocketAddr) -> Result<Self::TcpStream> {
        let s = self.0.tcp_connect(addr).await?;
        Ok((Box::new(s)))
    }
}

#[async_trait]
impl<T: ProxyTcpListener> ProxyTcpListener for Boxed<T>
where
    T: ProxyTcpListener + 'static,
    T::TcpStream: 'static,
    T::TcpListener: 'static,
{
    type TcpStream = BoxedTcpStream;
    type TcpListener = BoxedTcpListener<T::TcpStream>;

    #[inline(always)]
    async fn tcp_bind(&self, addr: SocketAddr) -> Result<Self::TcpListener> {
        let s = self.0.tcp_bind(addr).await?;
        Ok((Box::new(s)))
    }
}

#[async_trait]
impl<T> ProxyUdpSocket for Boxed<T>
where
    T: ProxyUdpSocket + 'static,
    T::UdpSocket: 'static,
{
    type UdpSocket = BoxedUdpSocket;

    #[inline(always)]
    async fn udp_bind(&self, addr: SocketAddr) -> Result<Self::UdpSocket> {
        let s = self.0.udp_bind(addr).await?;
        Ok((Box::new(s)))
    }
}

pub type BoxedProxyTcpStream = Box<dyn ProxyTcpStream<TcpStream = BoxedTcpStream>>;

pub trait ProxyBoxExt: Sized {
    #[inline(always)]
    fn boxed(self) -> Box<Boxed<Self>> {
        Box::new(Boxed(self))
    }
}

impl<T> ProxyBoxExt for T {}
