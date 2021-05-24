#[cfg(feature = "api_server")]
pub mod api_server;
mod config;
mod schema;
mod translate;
mod util;

use std::{path::Path, time::Duration};

use anyhow::Result;
use futures::{
    future::{ready, TryFutureExt},
    stream::{self, StreamExt, TryStreamExt},
    Stream,
};
use notify_stream::{notify::RecursiveMode, notify_stream};
pub use rabbit_digger;
use rabbit_digger::Registry;
use tokio::fs::read_to_string;

use crate::util::DebounceStreamExt;

pub fn plugin_loader(_cfg: &rabbit_digger::Config, registry: &mut Registry) -> Result<()> {
    #[cfg(feature = "ss")]
    registry.init_with_registry("ss", ss::init)?;
    #[cfg(feature = "trojan")]
    registry.init_with_registry("trojan", trojan::init)?;
    Ok(())
}

pub fn watch_config_stream(
    path: impl AsRef<Path>,
) -> Result<impl Stream<Item = Result<rabbit_digger::Config>>> {
    let path = path.as_ref().to_owned();
    let watch_stream = notify_stream(&path, RecursiveMode::Recursive)?;
    let watch_stream = stream::once(async { Ok(()) })
        .chain(
            watch_stream
                .try_filter(|e| ready(e.kind.is_modify()))
                .map(|_| Ok(()))
                .debounce(Duration::from_millis(100)),
        )
        .and_then(move |_| read_to_string(path.clone()).map_err(Into::into))
        .and_then(|s| ready(serde_yaml::from_str(&s).map_err(Into::into)))
        .and_then(config::post_process);

    Ok(watch_stream)
}
