use std::{io, net::SocketAddr};

use futures::{future::BoxFuture, FutureExt};
use rd_interface::{
    async_trait,
    prelude::*,
    registry::{NetFactory, NetRef},
    Address, Arc, INet, Result, TcpListener, TcpStream, UdpSocket,
};

type Resolver =
    Arc<dyn Fn(String, u16) -> BoxFuture<'static, io::Result<Vec<SocketAddr>>> + Send + Sync>;
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
        let resolve_net = (*config.resolve_net).clone();

        let resolver: Resolver = Arc::new(move |domain: String, port: u16| {
            let resolve_net = resolve_net.clone();
            async move {
                Ok(resolve_net
                    .lookup_host(&Address::Domain(domain, port))
                    .await?
                    .into_iter()
                    .filter(|i| (ipv4 && i.is_ipv4()) || (ipv6 && i.is_ipv6()))
                    .collect())
            }
            .boxed()
        });
        ResolveNet { config, resolver }
    }
}

#[async_trait]
impl INet for ResolveNet {
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> Result<TcpStream> {
        let addrs = addr.resolve(&*self.resolver).await?;
        let mut last_err = None;

        for addr in addrs {
            match self.config.net.tcp_connect(ctx, &addr.into()).await {
                Ok(stream) => return Ok(stream),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
            .into()
        }))
    }

    async fn tcp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> Result<TcpListener> {
        self.config.net.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut rd_interface::Context, addr: &Address) -> Result<UdpSocket> {
        let addrs = addr.resolve(&*self.resolver).await?;
        let mut last_err = None;

        for addr in addrs {
            match self.config.net.udp_bind(ctx, &addr.into()).await {
                Ok(udp) => return Ok(udp),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
            .into()
        }))
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
