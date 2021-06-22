use net::RawNet;
use rd_interface::{Registry, Result};
use server::RawServer;

mod device;
mod gateway;
mod net;
mod server;
mod tap;
mod wrap;

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<RawNet>();
    registry.add_server::<tap::TapServer>();
    registry.add_server::<RawServer>();

    Ok(())
}
