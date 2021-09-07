pub use dns_net::DNSNet;
use rd_interface::{
    prelude::*,
    rd_config,
    registry::{NetFactory, NetRef},
    Registry, Result,
};

mod dns_net;
mod service;

#[rd_config]
#[derive(Debug)]
pub struct DNSNetConfig {
    #[serde(default)]
    net: NetRef,
}

impl NetFactory for DNSNet {
    const NAME: &'static str = "dns";
    type Config = DNSNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(DNSNet::new(config.net.net()))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<DNSNet>();
    Ok(())
}
