use net::RemoteNet;
use protocol::get_protocol;
use rd_interface::{
    registry::{NetFactory, NetRef, ServerFactory},
    schemars::{self, JsonSchema},
    Config, Net, Registry, Result,
};
use serde_derive::Deserialize;
use server::RemoteServer;

mod net;
mod protocol;
mod server;

#[derive(Deserialize, JsonSchema, Config)]
pub struct RemoteNetConfig {
    #[serde(default)]
    net: NetRef,
    #[serde(flatten)]
    config: protocol::Config,
}

impl NetFactory for RemoteNet {
    const NAME: &'static str = "remote";
    type Config = RemoteNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        let protocol = get_protocol(config.net.net(), config.config)?;
        Ok(RemoteNet::new(protocol))
    }
}

impl ServerFactory for RemoteServer {
    const NAME: &'static str = "remote";
    type Config = protocol::Config;
    type Server = Self;

    fn new(listen: Net, net: Net, cfg: Self::Config) -> Result<Self> {
        let protocol = get_protocol(listen, cfg)?;
        Ok(RemoteServer::new(protocol, net))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<RemoteNet>();
    registry.add_server::<RemoteServer>();

    Ok(())
}
