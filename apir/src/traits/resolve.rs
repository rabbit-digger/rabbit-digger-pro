use super::{runtime::*, IntoAddress};
use std::io::Result;

pub struct Resolve<R, T> {
    proxy: T,
    resolver: R,
}

#[async_trait]
impl<R: ProxyResolver, T: ProxyTcpListener> ProxyTcpListener for Resolve<R, T> {
    type TcpStream = T::TcpStream;
    type TcpListener = T::TcpListener;

    #[inline(always)]
    async fn tcp_bind<A: IntoAddress>(&self, addr: A) -> Result<Self::TcpListener> {
        let addr = self.resolver.resolve(addr).await?;
        T::tcp_bind(&self.proxy, addr).await
    }
}

#[async_trait]
impl<R: ProxyResolver, T: ProxyTcpStream> ProxyTcpStream for Resolve<R, T> {
    type TcpStream = T::TcpStream;

    #[inline(always)]
    async fn tcp_connect<A: IntoAddress>(&self, addr: A) -> Result<Self::TcpStream> {
        let addr = self.resolver.resolve(addr).await?;
        T::tcp_connect(&self.proxy, addr).await
    }
}

#[async_trait]
impl<R: ProxyResolver, T: ProxyUdpSocket> ProxyUdpSocket for Resolve<R, T> {
    type UdpSocket = T::UdpSocket;

    #[inline(always)]
    async fn udp_bind<A: IntoAddress>(&self, addr: A) -> Result<Self::UdpSocket> {
        let addr = self.resolver.resolve(addr).await?;
        T::udp_bind(&self.proxy, addr).await
    }
}
