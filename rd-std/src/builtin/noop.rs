use rd_interface::{
    registry::{EmptyConfig, NetFactory},
    NotImplementedNet, Result,
};

pub struct NoopNet;

impl NetFactory for NoopNet {
    const NAME: &'static str = "noop";
    type Config = EmptyConfig;
    type Net = NotImplementedNet;

    fn new(_nets: Vec<rd_interface::Net>, _config: Self::Config) -> Result<Self::Net> {
        Ok(NotImplementedNet)
    }
}
