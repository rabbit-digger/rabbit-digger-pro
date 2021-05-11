pub mod alias;
pub mod combine;
pub mod forward;
pub mod local;

use crate::Registry;
use anyhow::Result;

pub fn load_builtin(registry: &mut Registry) -> Result<()> {
    let mut r = rd_interface::Registry::new();

    r.add_net::<alias::AliasNet>();
    r.add_net::<combine::CombineNet>();
    r.add_net::<local::LocalNet>();
    forward::init_plugin(&mut r)?;

    registry.add_registry("builtin".to_string(), r);
    Ok(())
}
