mod auth;
mod client;
mod common;
mod server;

pub use auth::NoAuth;
pub use client::Socks5Client;
pub use server::Socks5Server;

use rd_interface::{config::from_value, Registry, Result};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    address: String,
    port: u16,
}

#[no_mangle]
pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_net("socks5", |pr, cfg| {
        let Config { address, port } = from_value(cfg)?;
        Ok(Socks5Client::new(pr, address, port))
    });
    registry.add_server("socks5", |listen_net, net, cfg| {
        let Config { port, .. } = from_value(cfg)?;
        Ok(Socks5Server::new(listen_net, net, port))
    });
    Ok(())
}
