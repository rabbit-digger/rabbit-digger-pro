use futures::future::BoxFuture;
use rd_interface::{
    registry::{NetFactory, NetRef},
    Address, Config, Context, INet, Net, Result, TcpListener, TcpStream, UdpSocket,
};
use serde_derive::Deserialize;

pub struct CombineNet {
    tcp_connect: Net,
    tcp_bind: Net,
    udp_bind: Net,
}

impl INet for CombineNet {
    #[inline(always)]
    fn tcp_connect<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<TcpStream>>
    where
        Self: 'a,
    {
        self.tcp_connect.tcp_connect(ctx, addr)
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
        self.tcp_bind.tcp_bind(ctx, addr)
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
        self.udp_bind.udp_bind(ctx, addr)
    }
}

#[derive(Debug, Deserialize, Config)]
pub struct Config {
    tcp_connect: NetRef,
    tcp_bind: NetRef,
    udp_bind: NetRef,
}

impl NetFactory for CombineNet {
    const NAME: &'static str = "combine";
    type Config = Config;
    type Net = Self;

    fn new(
        Config {
            tcp_connect,
            tcp_bind,
            udp_bind,
        }: Self::Config,
    ) -> Result<Self> {
        Ok(CombineNet {
            tcp_connect: tcp_connect.net(),
            tcp_bind: tcp_bind.net(),
            udp_bind: udp_bind.net(),
        })
    }
}
