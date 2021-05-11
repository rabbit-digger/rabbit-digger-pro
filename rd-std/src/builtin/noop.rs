use rd_interface::{registry::NetFactory, NotImplementedNet, Result};

pub struct NoopNet;

impl NetFactory for NoopNet {
    const NAME: &'static str = "noop";
    type Config = ();
    type Net = NotImplementedNet;

    fn new(_nets: Vec<rd_interface::Net>, _config: Self::Config) -> Result<Self::Net> {
        Ok(NotImplementedNet)
    }
}
