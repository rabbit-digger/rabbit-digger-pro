pub use self::{client::HttpClient, server::HttpServer};

use rd_interface::{
    prelude::*,
    registry::{Builder, NetRef},
    Address, Net, Registry, Result, Server,
};

mod client;
mod server;
#[cfg(test)]
mod tests;

#[rd_config]
#[derive(Debug)]
pub struct HttpNetConfig {
    server: Address,

    #[serde(default)]
    net: NetRef,
}

#[rd_config]
#[derive(Debug)]
pub struct HttpServerConfig {
    bind: Address,
    #[serde(default)]
    net: NetRef,
    #[serde(default)]
    listen: NetRef,
}

impl Builder<Net> for HttpClient {
    const NAME: &'static str = "http";
    type Config = HttpNetConfig;
    type Item = Self;

    fn build(config: Self::Config) -> Result<Self> {
        Ok(HttpClient::new((*config.net).clone(), config.server))
    }
}

impl Builder<Server> for server::Http {
    const NAME: &'static str = "http";
    type Config = HttpServerConfig;
    type Item = Self;

    fn build(Self::Config { listen, net, bind }: Self::Config) -> Result<Self> {
        Ok(server::Http::new((*listen).clone(), (*net).clone(), bind))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<HttpClient>();
    registry.add_server::<server::Http>();
    Ok(())
}
