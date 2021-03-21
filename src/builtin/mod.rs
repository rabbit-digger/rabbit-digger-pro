pub mod alias;
pub mod local;
pub mod select;

use crate::Registry;
use anyhow::Result;

pub fn load_builtin(registry: &mut Registry) -> Result<()> {
    let mut r = rd_interface::Registry::new();

    alias::init_plugin(&mut r)?;
    local::init_plugin(&mut r)?;
    select::init_plugin(&mut r)?;

    registry.add_registry("builtin".to_string(), r);
    Ok(())
}
