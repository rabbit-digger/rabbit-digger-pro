use crate::registry::Registry;
use anyhow::Result;

pub fn load_builtin(registry: &mut Registry) -> Result<()> {
    #[cfg(feature = "rd-std")]
    registry.init_with_registry("std", rd_std::init)?;

    Ok(())
}
