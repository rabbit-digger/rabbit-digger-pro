pub mod alias;
pub mod combine;
pub mod forward;
pub mod local;

use crate::registry::Registry;
use anyhow::Result;

pub fn load_builtin(registry: &mut Registry) -> Result<()> {
    #[cfg(feature = "rd-socks5")]
    registry.init_with_registry("socks5", |r| rd_socks5::init(r).map_err(Into::into))?;
    #[cfg(feature = "rd-redir")]
    registry.init_with_registry("redir", |r| rd_redir::init(r).map_err(Into::into))?;

    registry.init_with_registry("builtin", |r| {
        r.add_net::<alias::AliasNet>();
        r.add_net::<combine::CombineNet>();
        r.add_net::<local::LocalNet>();

        r.add_server::<forward::ForwardNet>();

        Ok(())
    })
}
