pub use self::{client::Socks5Client, server::Socks5Server};

use rd_interface::{
    registry::{NetFactory, NetRef, ServerFactory},
    schemars::{self, JsonSchema},
    Address, Config, Net, Registry, Result,
};
use serde_derive::Deserialize;

mod client;
mod common;
mod server;
#[cfg(test)]
mod tests;

#[derive(Debug, Deserialize, Config, JsonSchema)]
pub struct Socks5NetConfig {
    server: Address,

    #[serde(default)]
    net: NetRef,
}

#[derive(Debug, Deserialize, Config, JsonSchema)]
pub struct Socks5ServerConfig {
    bind: Address,
}

impl NetFactory for Socks5Client {
    const NAME: &'static str = "socks5";
    type Config = Socks5NetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(Socks5Client::new(config.net.net(), config.server))
    }
}

impl ServerFactory for server::Socks5 {
    const NAME: &'static str = "socks5";
    type Config = Socks5ServerConfig;
    type Server = Self;

    fn new(listen: Net, net: Net, Self::Config { bind }: Self::Config) -> Result<Self> {
        Ok(server::Socks5::new(listen, net, bind))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<Socks5Client>();
    registry.add_server::<server::Socks5>();
    Ok(())
}
