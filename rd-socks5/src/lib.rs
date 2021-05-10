mod client;
mod common;
mod protocol;
mod server;
mod socks5_server;

pub use client::Socks5Client;
pub use socks5_server::Socks5Server;

use rd_interface::{config::from_value, util::get_one_net, Registry, Result};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    address: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    bind: String,
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net("socks5", |pr, cfg| {
        let Config { address, port } = from_value(cfg)?;
        Ok(Socks5Client::new(get_one_net(pr)?, address, port))
    });
    registry.add_server("socks5", |listen_net, net, cfg| {
        let ServerConfig { bind } = from_value(cfg)?;
        Ok(server::Socks5::new(listen_net, net, bind))
    });
    Ok(())
}

#[cfg(feature = "plugin")]
#[no_mangle]
pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    init(registry)
}
