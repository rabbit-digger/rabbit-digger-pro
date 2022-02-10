use net::RpcNet;
use rd_interface::{
    config::NetRef,
    prelude::*,
    rd_config,
    registry::{NetBuilder, ServerBuilder},
    Address, Net, Registry, Result,
};
use server::RpcServer;

mod connection;
mod net;
mod server;
mod types;

#[rd_config]
pub struct RpcNetConfig {
    #[serde(default)]
    net: NetRef,
    endpoint: Address,
}

#[rd_config]
pub struct RpcServerConfig {
    bind: Address,
}

impl NetBuilder for RpcNet {
    const NAME: &'static str = "rpc";
    type Config = RpcNetConfig;
    type Net = Self;

    fn build(config: Self::Config) -> Result<Self> {
        Ok(RpcNet::new((*config.net).clone(), config.endpoint))
    }
}

impl ServerBuilder for RpcServer {
    const NAME: &'static str = "rpc";
    type Config = RpcServerConfig;
    type Server = Self;

    fn build(listen: Net, net: Net, Self::Config { bind }: Self::Config) -> Result<Self> {
        Ok(RpcServer::new(listen, net, bind))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<RpcNet>();
    registry.add_server::<RpcServer>();

    Ok(())
}
