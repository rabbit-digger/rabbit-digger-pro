pub use crate::builtin::load_builtin;
use crate::registry::Registry;
use anyhow::Result;
use libloading::{Library, Symbol};
use std::{
    ffi::OsStr,
    fs::read_dir,
    path::{Path, PathBuf},
};

const PLUGIN_EXTENSIONS: &'static [&'static str] = &["so", "dll"];

pub fn load_plugin(path: &Path, registry: &mut rd_interface::Registry) -> Result<()> {
    log::trace!("Loading plugin: {:?}", path);
    let lib = Library::new(path)?;
    let init_plugin: Symbol<fn(&mut rd_interface::Registry) -> rd_interface::Result<()>> =
        unsafe { lib.get(b"init_plugin")? };
    init_plugin(registry)?;
    std::mem::forget(lib);

    Ok(())
}

pub fn load_plugins(path: PathBuf) -> Result<Registry> {
    let exts: Vec<&OsStr> = PLUGIN_EXTENSIONS.iter().map(|i| OsStr::new(i)).collect();
    let dirs = read_dir(&path);
    let mut registry = Registry::new();

    if dirs.is_err() {
        log::error!("Error when reading dir: {:?}", path);
        load_builtin(&mut registry)?;
        return Ok(registry);
    }

    #[cfg(not(feature = "disable_plugins"))]
    for i in dirs? {
        let p = i?.path();
        if !p.is_dir() && exts.contains(&p.extension().unwrap_or_default()) {
            let name = p.to_string_lossy().to_string();
            let r = registry.init_with_registry(name, |r| load_plugin(p.as_path(), r));
            if let Err(e) = r {
                log::warn!("Skip plugin: {} reason: {:?}", p.to_string_lossy(), e);
            }
        }
    }

    #[cfg(feature = "rd-socks5")]
    registry.init_with_registry("socks5", |r| rd_socks5::init(r).map_err(Into::into))?;
    #[cfg(feature = "rd-redir")]
    registry.init_with_registry("redir", |r| rd_redir::init(r).map_err(Into::into))?;

    // Prevent builtin plugins from being tampered with
    load_builtin(&mut registry)?;

    Ok(registry)
}
