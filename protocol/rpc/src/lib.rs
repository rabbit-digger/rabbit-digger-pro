use net::RpcNet;
use rd_interface::{
    config::NetRef, prelude::*, rd_config, registry::Builder, Address, Net, Registry, Result,
    Server,
};
use server::RpcServer;

mod connection;
mod net;
mod server;
mod session;
#[cfg(test)]
mod tests;
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

    #[serde(default)]
    net: NetRef,
    #[serde(default)]
    listen: NetRef,
}

impl Builder<Net> for RpcNet {
    const NAME: &'static str = "rpc";
    type Config = RpcNetConfig;
    type Item = Self;

    fn build(config: Self::Config) -> Result<Self> {
        Ok(RpcNet::new((*config.net).clone(), config.endpoint, true))
    }
}

impl Builder<Server> for RpcServer {
    const NAME: &'static str = "rpc";
    type Config = RpcServerConfig;
    type Item = Self;

    fn build(Self::Config { listen, net, bind }: Self::Config) -> Result<Self> {
        Ok(RpcServer::new((*listen).clone(), (*net).clone(), bind))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<RpcNet>();
    registry.add_server::<RpcServer>();

    Ok(())
}
