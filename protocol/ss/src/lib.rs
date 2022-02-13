use client::{SSNet, SSNetConfig};
use rd_interface::{
    registry::{NetBuilder, ServerBuilder},
    Registry, Result,
};
use server::{SSServer, SSServerConfig};

mod client;
mod server;
#[cfg(test)]
mod tests;
mod udp;
mod wrapper;

impl NetBuilder for SSNet {
    const NAME: &'static str = "shadowsocks";
    type Config = SSNetConfig;
    type Net = Self;

    fn build(config: Self::Config) -> Result<Self> {
        Ok(SSNet::new(config))
    }
}

impl ServerBuilder for SSServer {
    const NAME: &'static str = "shadowsocks";
    type Config = SSServerConfig;
    type Server = Self;

    fn build(cfg: Self::Config) -> Result<Self> {
        Ok(SSServer::new(cfg))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<SSNet>();
    registry.add_server::<SSServer>();

    Ok(())
}
