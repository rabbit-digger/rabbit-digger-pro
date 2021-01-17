use anyhow::Result;
use apir::dynamic::{BoxProxyNet, PluginInfo};
use itertools::process_results;
use libloading::{Library, Symbol};
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

pub fn load_plugin(path: PathBuf) -> Result<Plugin> {
    let lib = Library::new(path)?;
    let new_plugin: Symbol<fn() -> PluginInfo> = unsafe { lib.get(b"new_plugin")? };
    let PluginInfo { net, name } = new_plugin();

    Ok(Plugin { name, net, lib })
}

pub fn load_plugins() -> Result<HashMap<String, Plugin>> {
    let dirs = read_dir("plugins");
    if dirs.is_err() {
        return Ok(HashMap::new());
    }
    process_results(
        dirs?.into_iter().filter_map(|i| {
            let p = i.ok()?.path();
            if !p.is_dir() {
                Some(load_plugin(p))
            } else {
                None
            }
        }),
        |r| r.map(|i| (i.name.clone(), i)).collect(),
    )
}
