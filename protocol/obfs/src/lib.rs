use obfs_net::{ObfsNet, ObfsNetConfig};
use rd_interface::{registry::NetFactory, Registry, Result};

mod obfs_net;

impl NetFactory for ObfsNet {
    const NAME: &'static str = "obfs";

    type Config = ObfsNetConfig;

    type Net = ObfsNet;

    fn new(config: Self::Config) -> Result<Self::Net> {
        ObfsNet::new(config)
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<ObfsNet>();

    Ok(())
}
