mod client;
mod common;
mod server;

pub use client::Socks5Client;
pub use server::Socks5Server;

use rd_interface::{
    registry::{NetFactory, NetRef, ServerFactory},
    Config, Net, Registry, Result,
};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize, Config)]
pub struct ClientConfig {
    address: String,
    port: u16,

    #[serde(default)]
    net: NetRef,
}

#[derive(Debug, Deserialize, Config)]
pub struct ServerConfig {
    bind: String,
}

impl NetFactory for Socks5Client {
    const NAME: &'static str = "socks5";
    type Config = ClientConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(Socks5Client::new(
            config.net.net(),
            config.address,
            config.port,
        ))
    }
}

impl ServerFactory for server::Socks5 {
    const NAME: &'static str = "socks5";
    type Config = ServerConfig;
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
