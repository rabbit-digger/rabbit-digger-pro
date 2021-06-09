pub use self::{client::HttpClient, server::HttpServer};

use rd_interface::{
    registry::{NetFactory, NetRef, ServerFactory},
    schemars::{self, JsonSchema},
    Address, Config, Net, Registry, Result,
};
use serde_derive::Deserialize;

mod client;
mod server;
#[cfg(test)]
mod tests;

#[derive(Debug, Deserialize, Config, JsonSchema)]
pub struct ClientConfig {
    address: String,
    port: u16,

    #[serde(default)]
    net: NetRef,
}

#[derive(Debug, Deserialize, Config, JsonSchema)]
pub struct HttpServerConfig {
    bind: Address,
}

impl NetFactory for HttpClient {
    const NAME: &'static str = "http";
    type Config = ClientConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(HttpClient::new(
            config.net.net(),
            config.address,
            config.port,
        ))
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
