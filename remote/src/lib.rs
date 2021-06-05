use net::RemoteNet;
use protocol::get_protocol;
use rd_interface::{
    registry::{NetFactory, NetRef, ServerFactory},
    schemars::{self, JsonSchema},
    Config, Net, Registry, Result,
};
use serde_derive::Deserialize;

mod net;
mod protocol;
mod server;

#[derive(Deserialize, JsonSchema, Config)]
pub struct RemoteNetConfig {
    #[serde(default)]
    net: NetRef,
    config: protocol::Config,
}

impl NetFactory for RemoteNet {
    const NAME: &'static str = "shadowsocks";
    type Config = RemoteNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        let protocol = get_protocol(config.net.net(), config.config)?;
        Ok(RemoteNet::new(protocol))
    }
}

// impl ServerFactory for SSServer {
//     const NAME: &'static str = "shadowsocks";
//     type Config = SSServerConfig;
//     type Server = Self;

//     fn new(listen: Net, net: Net, cfg: Self::Config) -> Result<Self> {
//         Ok(SSServer::new(listen, net, cfg))
//     }
// }

pub fn init(registry: &mut Registry) -> Result<()> {
    // registry.add_net::<SSNet>();
    // registry.add_server::<SSServer>();

    Ok(())
}
