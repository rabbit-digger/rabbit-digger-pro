use net::RpcNet;
use rd_interface::{
    config::NetRef, prelude::*, rd_config, registry::NetBuilder, Address, Registry, Result,
};

mod connection;
mod net;
mod types;

#[rd_config]
pub struct RpcNetConfig {
    #[serde(default)]
    net: NetRef,
    endpoint: Address,
}

impl NetBuilder for RpcNet {
    const NAME: &'static str = "rpc";
    type Config = RpcNetConfig;
    type Net = Self;

    fn build(config: Self::Config) -> Result<Self> {
        Ok(RpcNet::new((*config.net).clone(), config.endpoint))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<RpcNet>();

    Ok(())
}
