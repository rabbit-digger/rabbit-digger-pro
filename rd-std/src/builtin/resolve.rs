use std::{
    io::{self, ErrorKind},
    net::SocketAddr,
};

use futures::{future::BoxFuture, FutureExt};
use rd_interface::{
    async_trait,
    prelude::*,
    registry::{NetFactory, NetRef},
    Address, Arc, INet, IntoDyn, Result, TcpListener, TcpStream, UdpSocket,
};

type Resolver =
    Arc<dyn Fn(String, u16) -> BoxFuture<'static, io::Result<SocketAddr>> + Send + Sync>;
pub struct Udp(UdpSocket, Resolver);

// Resolves domain names to IP addresses before connecting.
#[rd_config]
#[derive(Debug)]
pub struct ResolveConfig {
    net: NetRef,
    resolve_net: NetRef,
    #[serde(default = "bool_true")]
    ipv4: bool,
    #[serde(default = "bool_true")]
    ipv6: bool,
}

fn bool_true() -> bool {
    true
}

pub struct ResolveNet {
    config: ResolveConfig,
    resolver: Resolver,
}

impl ResolveNet {
    pub fn new(config: ResolveConfig) -> ResolveNet {
        let (ipv4, ipv6) = (config.ipv4, config.ipv6);
        let resolve_net = config.resolve_net.net();

        let resolver: Resolver = Arc::new(move |domain: String, port: u16| {
            let resolve_net = resolve_net.clone();
            async move {
                resolve_net
                    .lookup_host(&Address::Domain(domain, port))
                    .await?
                    .into_iter()
                    .find(|i| (ipv4 && i.is_ipv4()) || (ipv6 && i.is_ipv6()))
                    .ok_or_else(|| io::Error::from(ErrorKind::AddrNotAvailable))
            }
            .boxed()
        });
        ResolveNet { config, resolver }
    }
}

#[async_trait]
impl rd_interface::IUdpSocket for Udp {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        self.0.recv_from(buf).await.map_err(Into::into)
    }

    async fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize> {
        let addr = addr.resolve(&*self.1).await?;
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
        addr: &Address,
    ) -> Result<TcpStream> {
        let addr = addr.resolve(&*self.resolver).await?;
        let tcp = self.config.net.tcp_connect(ctx, &addr.into()).await?;
        Ok(tcp)
    }

    async fn tcp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> Result<TcpListener> {
        self.config.net.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut rd_interface::Context, addr: &Address) -> Result<UdpSocket> {
        let addr = addr.resolve(&*self.resolver).await?;
        let udp = self.config.net.udp_bind(ctx, &addr.into()).await?;
        Ok(Udp(udp, self.resolver.clone()).into_dyn())
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
