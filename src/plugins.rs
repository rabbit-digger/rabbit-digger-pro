use crate::registry::Registry;
use anyhow::Result;
use libloading::{Library, Symbol};
use std::{ffi::OsStr, fs::read_dir, path::PathBuf};

const PLUGIN_EXTENSIONS: &'static [&'static str] = &["so", "dll"];

pub fn load_plugin(path: PathBuf, registry: &mut rd_interface::Registry) -> Result<()> {
    let lib = Library::new(path)?;
    let init_plugin: Symbol<fn(&mut rd_interface::Registry) -> rd_interface::Result<()>> =
        unsafe { lib.get(b"init_plugin")? };
    init_plugin(registry)?;
    std::mem::forget(lib);

    Ok(())
}

pub fn load_plugins(path: PathBuf) -> Result<Registry> {
    let exts: Vec<&OsStr> = PLUGIN_EXTENSIONS.iter().map(|i| OsStr::new(i)).collect();
    let dirs = read_dir(path);
    if dirs.is_err() {
        return Ok(Registry::new());
    }

    let mut registry = Registry::new();
    for i in dirs? {
        let p = i?.path();
        if !p.is_dir() && exts.contains(&p.extension().unwrap_or_default()) {
            let mut r = rd_interface::Registry::new();
            load_plugin(p.clone(), &mut r)?;
            registry.add_registry(p.to_string_lossy().to_string(), r);
        }
    }

    Ok(registry)
}
