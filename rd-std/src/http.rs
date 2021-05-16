use rd_interface::{registry::ServerFactory, Config, Net, Registry, Result};
use serde_derive::Deserialize;
pub use server::HttpServer;

mod server;

#[derive(Debug, Deserialize, Config)]
pub struct ServerConfig {
    bind: String,
}

impl ServerFactory for server::Http {
    const NAME: &'static str = "http";
    type Config = ServerConfig;
    type Server = Self;

    fn new(listen: Net, net: Net, Self::Config { bind }: Self::Config) -> Result<Self> {
        Ok(server::Http::new(listen, net, bind))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_server::<server::Http>();
    Ok(())
}
