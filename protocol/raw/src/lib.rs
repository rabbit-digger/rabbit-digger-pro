use net::RawNet;
use rd_interface::{Registry, Result};

mod config;
mod device;
mod gateway;
mod net;
// #[cfg(unix)]
// mod unix;
mod forward;
mod wrap;

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<RawNet>();

    Ok(())
}
