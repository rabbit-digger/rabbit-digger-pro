use anyhow::Result;
use itertools::process_results;
use libloading::{Library, Symbol};
use rd_interface::{BoxProxyNet, Registry};
use std::{collections::HashMap, fmt, fs::read_dir, path::PathBuf};

pub struct Plugin {
    name: String,
    net: BoxProxyNet,
    lib: Library,
}
impl fmt::Debug for Plugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Plugin").field("name", &self.name).finish()
    }
}

pub fn load_plugin(path: PathBuf, registry: &mut Registry) -> Result<()> {
    let lib = Library::new(path)?;
    let init_plugin: Symbol<fn(&mut Registry) -> rd_interface::Result<()>> =
        unsafe { lib.get(b"init_plugin")? };
    init_plugin(registry)?;
    std::mem::forget(lib);

    Ok(())
}

pub fn load_plugins() -> Result<Registry> {
    let dirs = read_dir("plugins");
    if dirs.is_err() {
        return Ok(Registry::new());
    }

    let mut registry = Registry::new();
    for i in dirs? {
        let p = i?.path();
        if !p.is_dir() {
            load_plugin(p, &mut registry)?;
        }
    }

    Ok(registry)
}
