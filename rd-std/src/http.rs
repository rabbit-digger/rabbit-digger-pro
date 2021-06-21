pub use self::{client::HttpClient, server::HttpServer};

use rd_interface::{
    prelude::*,
    registry::{NetFactory, NetRef, ServerFactory},
    Address, Net, Registry, Result,
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
}

impl NetFactory for HttpClient {
    const NAME: &'static str = "http";
    type Config = HttpNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(HttpClient::new(config.net.net(), config.server))
    }
}

impl ServerFactory for server::Http {
    const NAME: &'static str = "http";
    type Config = HttpServerConfig;
    type Server = Self;

    fn new(listen: Net, net: Net, Self::Config { bind }: Self::Config) -> Result<Self> {
        Ok(server::Http::new(listen, net, bind))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<HttpClient>();
    registry.add_server::<server::Http>();
    Ok(())
}
