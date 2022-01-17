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

#[cfg(test)]
mod tests {
    use rd_interface::IntoDyn;

    use super::*;
    use crate::tests::{
        assert_echo, assert_echo_udp, spawn_echo_server, spawn_echo_server_udp, TestNet,
    };

    #[tokio::test]
    async fn test_combine_net() {
        let net1 = TestNet::new().into_dyn();
        let net2 = TestNet::new().into_dyn();
        let net3 = TestNet::new().into_dyn();
        let net = CombineNet {
            tcp_connect: net1.clone(),
            tcp_bind: net2.clone(),
            udp_bind: net3.clone(),
            lookup_host: net1.clone(),
        }
        .into_dyn();

        spawn_echo_server(&net, "127.0.0.1:26666").await;
        assert_echo(&net2, "127.0.0.1:26666").await;

        spawn_echo_server_udp(&net, "127.0.0.1:26666").await;
        assert_echo_udp(&net3, "127.0.0.1:26666").await;
    }
}
