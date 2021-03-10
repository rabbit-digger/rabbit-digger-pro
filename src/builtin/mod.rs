pub mod local;

use crate::Registry;
use anyhow::Result;

pub fn load_builtin(registry: &mut Registry) -> Result<()> {
    let mut r = rd_interface::Registry::new();

    local::init_plugin(&mut r)?;

    registry.add_registry("builtin".to_string(), r);
    Ok(())
}
