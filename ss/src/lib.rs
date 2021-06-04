use client::{SSNet, SSNetConfig};
use rd_interface::{registry::NetFactory, Registry, Result};

mod client;
mod udp;
mod wrapper;

impl NetFactory for SSNet {
    const NAME: &'static str = "shadowsocks";
    type Config = SSNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(SSNet::new(config))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<SSNet>();

    Ok(())
}
