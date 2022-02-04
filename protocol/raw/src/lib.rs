use net::RawNet;
use rd_interface::{Registry, Result};

mod config;
mod device;
mod forward;
mod gateway;
mod net;
mod wrap;

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<RawNet>();

    Ok(())
}
