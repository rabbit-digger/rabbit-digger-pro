use anyhow::Result;
use libloading::{Library, Symbol};
use rd_interface::Registry;
use std::{ffi::OsStr, fs::read_dir, path::PathBuf};

const PLUGIN_EXTENSIONS: &'static [&'static str] = &["so", "dll"];

pub fn load_plugin(path: PathBuf, registry: &mut Registry) -> Result<()> {
    let lib = Library::new(path)?;
    let init_plugin: Symbol<fn(&mut Registry) -> rd_interface::Result<()>> =
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
            load_plugin(p, &mut registry)?;
        }
    }

    Ok(registry)
}
