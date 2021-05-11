use crate::registry::Registry;
use anyhow::Result;

pub fn load_builtin(registry: &mut Registry) -> Result<()> {
    #[cfg(feature = "rd-std")]
    registry.init_with_registry("std", |r| rd_std::init(r).map_err(Into::into))?;

    Ok(())
}
