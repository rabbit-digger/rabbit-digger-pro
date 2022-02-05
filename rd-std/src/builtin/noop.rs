use crate::util::NotImplementedNet;
use rd_interface::{config::EmptyConfig, registry::NetBuilder, Result};

pub struct NoopNet;

impl NetBuilder for NoopNet {
    const NAME: &'static str = "noop";
    type Config = EmptyConfig;
    type Net = NotImplementedNet;

    fn build(_config: Self::Config) -> Result<Self::Net> {
        Ok(NotImplementedNet)
    }
}
