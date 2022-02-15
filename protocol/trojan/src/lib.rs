use client::{TrojanNet, TrojanNetConfig};
use rd_interface::{registry::Builder, Net, Registry, Result};

mod client;
mod stream;
mod tls;
mod websocket;

impl Builder<Net> for TrojanNet {
    const NAME: &'static str = "trojan";
    type Config = TrojanNetConfig;
    type Item = Self;

    fn build(config: Self::Config) -> Result<Self> {
        TrojanNet::new(config)
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<TrojanNet>();

    Ok(())
}
