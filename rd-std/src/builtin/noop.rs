use crate::util::NotImplementedNet;
use rd_interface::{config::EmptyConfig, registry::NetFactory, Result};

pub struct NoopNet;

impl NetFactory for NoopNet {
    const NAME: &'static str = "noop";
    type Config = EmptyConfig;
    type Net = NotImplementedNet;

    fn new(_config: Self::Config) -> Result<Self::Net> {
        Ok(NotImplementedNet)
    }
}
