use std::{
    io::{self, ErrorKind},
    net::SocketAddr,
};

use rd_interface::{
    async_trait,
    registry::{NetFactory, NetRef},
    schemars::{self, JsonSchema},
    Address, Config, INet, IntoDyn, Result, TcpListener, TcpStream, UdpSocket,
};
use serde_derive::Deserialize;

pub struct Udp(UdpSocket);

#[derive(Debug, Deserialize, Config, JsonSchema)]
pub struct ResolveConfig {
    net: NetRef,
}

pub struct ResolveNet(ResolveConfig);

impl ResolveNet {
    pub fn new(config: ResolveConfig) -> ResolveNet {
        ResolveNet(config)
    }
}

async fn lookup_host(domain: String, port: u16) -> io::Result<SocketAddr> {
    use tokio::net::lookup_host;

    let domain = (domain.as_ref(), port);
    lookup_host(domain)
        .await?
        .next()
        .ok_or(ErrorKind::AddrNotAvailable.into())
}

#[async_trait]
impl rd_interface::IUdpSocket for Udp {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        self.0.recv_from(buf).await.map_err(Into::into)
    }

    async fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize> {
        let addr = addr.resolve(lookup_host).await?;
        self.0.send_to(buf, addr.into()).await.map_err(Into::into)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.local_addr().await?)
    }
}

#[async_trait]
impl INet for ResolveNet {
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: Address,
    ) -> Result<TcpStream> {
        let addr = addr.resolve(lookup_host).await?;
        let tcp = self.0.net.tcp_connect(ctx, addr.into()).await?;
        Ok(tcp)
    }

    async fn tcp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: Address,
    ) -> Result<TcpListener> {
        self.0.net.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut rd_interface::Context, addr: Address) -> Result<UdpSocket> {
        let addr = addr.resolve(lookup_host).await?;
        let udp = self.0.net.udp_bind(ctx, addr.into()).await?;
        Ok(Udp(udp).into_dyn())
    }
}

impl NetFactory for ResolveNet {
    const NAME: &'static str = "resolve";
    type Config = ResolveConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(ResolveNet::new(config))
    }
}
