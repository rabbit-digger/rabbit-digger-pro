pub mod alias;
pub mod combine;
pub mod forward;
pub mod local;

use crate::registry::Registry;
use anyhow::Result;

pub fn load_builtin(registry: &mut Registry) -> Result<()> {
    #[cfg(feature = "rd-std")]
    registry.init_with_registry("std", |r| rd_std::init(r).map_err(Into::into))?;

    registry.init_with_registry("builtin", |r| {
        r.add_net::<alias::AliasNet>();
        r.add_net::<combine::CombineNet>();
        r.add_net::<local::LocalNet>();

        r.add_server::<forward::ForwardNet>();

        Ok(())
    })
}
