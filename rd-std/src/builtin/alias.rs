use std::net::SocketAddr;

use rd_interface::{
    async_trait, config::NetRef, prelude::*, registry::NetFactory, Address, Context, INet, Result,
    TcpListener, TcpStream, UdpSocket,
};

pub struct AliasNet(rd_interface::Net);

impl AliasNet {
    fn new(net: rd_interface::Net) -> AliasNet {
        AliasNet(net)
    }
}

#[async_trait]
impl INet for AliasNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: &Address) -> Result<TcpStream> {
        self.0.tcp_connect(ctx, addr).await
    }

    async fn tcp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<TcpListener> {
        self.0.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        self.0.udp_bind(ctx, addr).await
    }

    async fn lookup_host(&self, addr: &Address) -> Result<Vec<SocketAddr>> {
        self.0.lookup_host(addr).await
    }
}

/// A net refering to another net.
#[rd_config]
#[derive(Debug)]
pub struct AliasNetConfig {
    net: NetRef,
}

impl NetFactory for AliasNet {
    const NAME: &'static str = "alias";
    type Config = AliasNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(AliasNet::new((*config.net).clone()))
    }
}
