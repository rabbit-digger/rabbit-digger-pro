use net::RawNet;
use rd_interface::{Registry, Result};

mod device;
mod net;

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<RawNet>();

    Ok(())
}
