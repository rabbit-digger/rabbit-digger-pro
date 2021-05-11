use rd_interface::{
    registry::{NetFactory, ServerFactory},
    util::get_one_net,
    Net, Registry, Result,
};
use serde_derive::Deserialize;

mod server;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    bind: String,
}

impl ServerFactory for server::Http {
    const NAME: &'static str = "http";
    type Config = ServerConfig;

    fn new(listen_net: Net, net: Net, Self::Config { bind }: Self::Config) -> Result<Self> {
        Ok(server::Http::new(listen_net, net, bind))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_server::<server::Http>();
    Ok(())
}
