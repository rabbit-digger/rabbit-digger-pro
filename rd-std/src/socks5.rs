pub use self::{client::Socks5Client, server::Socks5Server};

use rd_interface::{
    prelude::*,
    registry::{NetBuilder, NetRef, ServerBuilder},
    Address, Registry, Result,
};

mod client;
mod common;
mod server;
#[cfg(test)]
mod tests;

#[rd_config]
#[derive(Debug)]
pub struct Socks5NetConfig {
    server: Address,

    #[serde(default)]
    net: NetRef,
}

#[rd_config]
#[derive(Debug)]
pub struct Socks5ServerConfig {
    bind: Address,

    #[serde(default)]
    net: NetRef,
    #[serde(default)]
    listen: NetRef,
}

impl NetBuilder for Socks5Client {
    const NAME: &'static str = "socks5";
    type Config = Socks5NetConfig;
    type Net = Self;

    fn build(config: Self::Config) -> Result<Self> {
        Ok(Socks5Client::new((*config.net).clone(), config.server))
    }
}

impl ServerBuilder for server::Socks5 {
    const NAME: &'static str = "socks5";
    type Config = Socks5ServerConfig;
    type Server = Self;

    fn build(Self::Config { listen, net, bind }: Self::Config) -> Result<Self> {
        Ok(server::Socks5::new((*listen).clone(), (*net).clone(), bind))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<Socks5Client>();
    registry.add_server::<server::Socks5>();
    Ok(())
}
