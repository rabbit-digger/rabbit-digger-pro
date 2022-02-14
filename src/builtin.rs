use rd_interface::Result;

use crate::registry::Registry;

pub fn load_builtin(registry: &mut Registry) -> Result<()> {
    #[cfg(feature = "rd-std")]
    registry.init_with_registry("std", rd_std::init)?;

    Ok(())
}
