use std::net::SocketAddr;

use rd_interface::{
    async_trait, config::NetRef, prelude::*, registry::NetFactory, Address, Context, INet, Net,
    Result, TcpListener, TcpStream, UdpSocket,
};

pub struct CombineNet {
    tcp_connect: Net,
    tcp_bind: Net,
    udp_bind: Net,
    lookup_host: Net,
}

#[async_trait]
impl INet for CombineNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: &Address) -> Result<TcpStream> {
        self.tcp_connect.tcp_connect(ctx, addr).await
    }

    async fn tcp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<TcpListener> {
        self.tcp_bind.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        self.udp_bind.udp_bind(ctx, addr).await
    }

    async fn lookup_host(&self, addr: &Address) -> Result<Vec<SocketAddr>> {
        self.lookup_host.lookup_host(addr).await
    }
}

/// CombineNet merges multiple nets into one.
#[rd_config]
#[derive(Debug)]
pub struct CombineNetConfig {
    tcp_connect: NetRef,
    tcp_bind: NetRef,
    udp_bind: NetRef,
    lookup_host: NetRef,
}

impl NetFactory for CombineNet {
    const NAME: &'static str = "combine";
    type Config = CombineNetConfig;
    type Net = Self;

    fn new(
        CombineNetConfig {
            tcp_connect,
            tcp_bind,
            udp_bind,
            lookup_host,
        }: Self::Config,
    ) -> Result<Self> {
        Ok(CombineNet {
            tcp_connect: (*tcp_connect).clone(),
            tcp_bind: (*tcp_bind).clone(),
            udp_bind: (*udp_bind).clone(),
            lookup_host: (*lookup_host).clone(),
        })
    }
}
