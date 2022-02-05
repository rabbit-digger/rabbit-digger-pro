pub use dns_sniffer::DNSSnifferNet;
use rd_interface::{
    prelude::*,
    rd_config,
    registry::{NetBuilder, NetRef},
    Registry, Result,
};

mod dns_sniffer;
mod service;

#[rd_config]
#[derive(Debug)]
pub struct DNSNetConfig {
    #[serde(default)]
    net: NetRef,
}

impl NetBuilder for DNSSnifferNet {
    const NAME: &'static str = "dns_sniffer";
    type Config = DNSNetConfig;
    type Net = Self;

    fn build(config: Self::Config) -> Result<Self> {
        Ok(DNSSnifferNet::new((*config.net).clone()))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<DNSSnifferNet>();
    Ok(())
}
