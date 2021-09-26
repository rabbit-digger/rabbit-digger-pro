use net::RawNet;
use rd_interface::{Registry, Result};
use server::RawServer;

mod config;
mod device;
mod gateway;
mod interface_info;
mod net;
mod server;
#[cfg(unix)]
mod unix;
mod wrap;

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<RawNet>();
    #[cfg(unix)]
    registry.add_server::<unix::tap::TapServer>();
    #[cfg(unix)]
    registry.add_server::<unix::tun::TunServer>();
    registry.add_server::<RawServer>();

    Ok(())
}
