#[cfg(feature = "api_server")]
pub mod api_server;
mod config;
pub mod schema;
mod translate;
mod util;

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use futures::{
    future::ready,
    stream::{self, StreamExt, TryStreamExt},
    Stream,
};
use notify_stream::{notify::RecursiveMode, notify_stream};
pub use rabbit_digger;
use rabbit_digger::Registry;
use tokio::fs::read_to_string;
use yaml_merge_keys::merge_keys_serde;

use crate::util::DebounceStreamExt;

pub fn plugin_loader(_cfg: &rabbit_digger::Config, registry: &mut Registry) -> Result<()> {
    #[cfg(feature = "ss")]
    registry.init_with_registry("ss", ss::init)?;
    #[cfg(feature = "trojan")]
    registry.init_with_registry("trojan", trojan::init)?;
    Ok(())
}

pub fn deserialize_config(_path: &Path, s: &str) -> Result<config::ConfigExt> {
    let raw_yaml = serde_yaml::from_str(&s)?;
    let merged = merge_keys_serde(raw_yaml)?;
    Ok(serde_yaml::from_value(merged)?)
}

pub async fn read_config(path: PathBuf) -> Result<rabbit_digger::Config> {
    let s = read_to_string(&path).await?;
    let config = deserialize_config(path.as_path(), &s)?;
    config::post_process(config).await
}

pub fn watch_config_stream(
    path: impl AsRef<Path>,
) -> Result<impl Stream<Item = Result<rabbit_digger::Config>>> {
    let path = path.as_ref().to_owned();
    let watch_stream = notify_stream(&path, RecursiveMode::Recursive)?
        .try_filter(|e| ready(e.kind.is_modify()))
        .map(|_| Ok(()))
        .debounce(Duration::from_millis(100));
    let watch_stream = stream::once(async { Ok(()) })
        .chain(watch_stream)
        .and_then(move |_| read_config(path.clone()));

    Ok(watch_stream)
}
