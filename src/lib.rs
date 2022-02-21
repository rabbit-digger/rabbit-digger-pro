use anyhow::Result;
pub use rabbit_digger;
use rabbit_digger::Registry;
use yaml_merge_keys::merge_keys_serde;

#[cfg(feature = "api_server")]
pub mod api_server;
pub mod config;
pub mod log;
pub mod schema;
mod select;
pub mod storage;
mod translate;
mod util;

pub fn get_registry() -> Result<Registry> {
    let mut registry = Registry::new_with_builtin()?;

    #[cfg(feature = "ss")]
    registry.init_with_registry("ss", ss::init)?;
    #[cfg(feature = "trojan")]
    registry.init_with_registry("trojan", trojan::init)?;
    #[cfg(feature = "rpc")]
    registry.init_with_registry("rpc", rpc::init)?;
    #[cfg(feature = "raw")]
    registry.init_with_registry("raw", raw::init)?;
    #[cfg(feature = "obfs")]
    registry.init_with_registry("obfs", obfs::init)?;

    registry.init_with_registry("rabbit-digger-pro", select::init)?;

    Ok(registry)
}

pub fn deserialize_config(s: &str) -> Result<config::ConfigExt> {
    let raw_yaml = serde_yaml::from_str(s)?;
    let merged = merge_keys_serde(raw_yaml)?;
    Ok(serde_yaml::from_value(merged)?)
}
