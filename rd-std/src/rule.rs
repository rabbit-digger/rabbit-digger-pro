mod any;
pub mod config;
mod domain;
mod ip_cidr;
mod matcher;
mod rule_net;
mod udp;

use rd_interface::{registry::NetFactory, Registry, Result};

impl NetFactory for rule_net::RuleNet {
    const NAME: &'static str = "rule";
    type Config = config::RuleNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        rule_net::RuleNet::new(config)
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<rule_net::RuleNet>();
    Ok(())
}
