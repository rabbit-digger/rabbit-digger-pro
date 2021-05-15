use futures::future::BoxFuture;
use rd_interface::{
    registry::{NetFactory, NetRef},
    Address, Config, Context, INet, Result, TcpListener, TcpStream, UdpSocket,
};
use serde_derive::Deserialize;

pub struct AliasNet(rd_interface::Net);

impl AliasNet {
    fn new(net: rd_interface::Net) -> AliasNet {
        AliasNet(net)
    }
}

impl INet for AliasNet {
    #[inline(always)]
    fn tcp_connect<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<TcpStream>>
    where
        Self: 'a,
    {
        self.0.tcp_connect(ctx, addr)
    }

    #[inline(always)]
    fn tcp_bind<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<TcpListener>>
    where
        Self: 'a,
    {
        self.0.tcp_bind(ctx, addr)
    }

    #[inline(always)]
    fn udp_bind<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<UdpSocket>>
    where
        Self: 'a,
    {
        self.0.udp_bind(ctx, addr)
    }
}

#[derive(Debug, Deserialize, Config)]
pub struct Config {
    net: NetRef,
}

impl NetFactory for AliasNet {
    const NAME: &'static str = "alias";
    type Config = Config;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(AliasNet::new(config.net.net()))
    }
}
