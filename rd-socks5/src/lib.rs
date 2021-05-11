mod client;
mod common;
mod http_server;
mod protocol;
mod server;
mod socks5_server;

pub use client::Socks5Client;
pub use socks5_server::Socks5Server;

use rd_interface::{config::from_value, registry::NetFactory, util::get_one_net, Registry, Result};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    address: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    bind: String,
}

impl NetFactory for Socks5Client {
    const NAME: &'static str = "socks5";
    type Config = Config;

    fn new(net: Vec<rd_interface::Net>, config: Self::Config) -> Result<Self> {
        Ok(Socks5Client::new(
            get_one_net(net)?,
            config.address,
            config.port,
        ))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<Socks5Client>();
    registry.add_server("socks5", |listen_net, net, cfg| {
        let ServerConfig { bind } = from_value(cfg)?;
        Ok(server::Socks5::new(listen_net, net, bind))
    });
    registry.add_server("http", |listen_net, net, cfg| {
        let ServerConfig { bind } = from_value(cfg)?;
        Ok(server::Http::new(listen_net, net, bind))
    });
    Ok(())
}
