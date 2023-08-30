pub use dns_sniffer::DNSSnifferNet;
pub use sni_sniffer::SNISnifferNet;

use rd_interface::{
    prelude::*,
    rd_config,
    registry::{Builder, NetRef},
    Net, Registry, Result,
};

mod dns_sniffer;
mod service;
mod sni_sniffer;

#[rd_config]
#[derive(Debug)]
pub struct DNSNetConfig {
    #[serde(default)]
    net: NetRef,
}

impl Builder<Net> for DNSSnifferNet {
    const NAME: &'static str = "dns_sniffer";
    type Config = DNSNetConfig;
    type Item = Self;

    fn build(config: Self::Config) -> Result<Self> {
        Ok(DNSSnifferNet::new(config.net.value_cloned()))
    }
}

#[rd_config]
#[derive(Debug)]
pub struct SNINetConfig {
    #[serde(default)]
    net: NetRef,
}

impl Builder<Net> for SNISnifferNet {
    const NAME: &'static str = "sni_sniffer";
    type Config = SNINetConfig;
    type Item = Self;

    fn build(config: Self::Config) -> Result<Self> {
        Ok(SNISnifferNet::new(config.net.value_cloned()))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<DNSSnifferNet>();
    registry.add_net::<SNISnifferNet>();
    Ok(())
}
