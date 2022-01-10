pub use dns_sniffer::DNSSnifferNet;
use rd_interface::{
    prelude::*,
    rd_config,
    registry::{NetFactory, NetRef},
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

impl NetFactory for DNSSnifferNet {
    const NAME: &'static str = "dns_sniffer";
    type Config = DNSNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(DNSSnifferNet::new((*config.net).clone()))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<DNSSnifferNet>();
    Ok(())
}
