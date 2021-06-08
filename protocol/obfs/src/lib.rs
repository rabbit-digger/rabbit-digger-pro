use bytes::{Bytes, BytesMut};
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

/// An obfs protocol used in front of a `TcpStream`
pub trait Obfs {
    /// Return the prefix of the stream.
    fn encode(&mut self) -> Result<Bytes>;
    /// Strip the prefix of the stream. Return true if get all the prefix.
    fn decode(&mut self, src: &mut BytesMut) -> Result<bool>;
}
