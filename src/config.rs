use anyhow::{Context, Result};
pub use cache::ConfigCache;
use futures::StreamExt;
pub use manager::ConfigManager;
use notify_stream::{notify::RecursiveMode, notify_stream};
use rabbit_digger::Config;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    future::pending,
    path::PathBuf,
    time::{Duration, SystemTime},
};
use tokio::{fs::read_to_string, time::sleep};

use crate::util::DebounceStreamExt;

mod cache;
mod manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportUrl {
    pub url: String,
    pub interval: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ImportSource {
    Path(PathBuf),
    Url(ImportUrl),
}

impl ImportSource {
    pub fn cache_key(&self) -> String {
        match self {
            ImportSource::Path(path) => format!("path:{:?}", path),
            ImportSource::Url(url) => format!("url:{}", url.url),
        }
    }
    pub async fn get_content(&self, cache: &dyn ConfigCache) -> Result<String> {
        let key = self.cache_key();
        let content = cache.get(&key).await?;

        if let Some(content) = content
            .map(|c| {
                self.get_expire_duration()
                    .map(|d| SystemTime::now() < c.updated_at + d)
                    .unwrap_or(true)
                    .then(move || c.content)
            })
            .flatten()
        {
            return Ok(content);
        }

        Ok(match self {
            ImportSource::Path(path) => read_to_string(path).await?,
            ImportSource::Url(ImportUrl { url, .. }) => {
                tracing::info!("Fetching {}", url);
                let content = reqwest::get(url).await?.text().await?;
                tracing::info!("Done");
                cache.set(&key, &content).await?;
                content
            }
        })
    }
    fn get_expire_duration(&self) -> Option<Duration> {
        match self {
            ImportSource::Path(_) => None,
            ImportSource::Url(ImportUrl { interval, .. }) => {
                interval.map(|i| Duration::from_secs(i))
            }
        }
    }
    pub async fn wait(&self, cache: &dyn ConfigCache) -> Result<()> {
        match self {
            ImportSource::Path(path) => {
                let mut stream = notify_stream(path, RecursiveMode::NonRecursive)?
                    .debounce(Duration::from_millis(100));
                stream.next().await;
            }
            ImportSource::Url(ImportUrl { interval, .. }) => {
                let updated_at = cache.get_updated_at(&self.cache_key()).await?;
                match (updated_at, interval) {
                    (None, _) => {}
                    (Some(_), None) => pending().await,
                    (Some(updated_at), Some(interval)) => {
                        let expired_at = updated_at + Duration::from_secs(*interval);
                        let tts = expired_at
                            .duration_since(SystemTime::now())
                            .unwrap_or_else(|_| Duration::ZERO);
                        sleep(tts).await
                    }
                }
            }
        };
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Import {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub format: String,
    #[serde(flatten)]
    pub(super) source: ImportSource,
    #[serde(flatten)]
    pub opt: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ConfigImport {
    #[serde(default)]
    import: Vec<Import>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigExt {
    #[serde(flatten)]
    config: Config,
    #[serde(default)]
    import: Vec<Import>,
}

impl ConfigExt {
    pub async fn build_from_cache(self, cache: &dyn cache::ConfigCache) -> Result<Config> {
        let imports = self.import;
        let mut config = self.config;
        for i in imports {
            let mut temp_config = Config::default();
            crate::translate::post_process(&mut temp_config, i.clone(), cache)
                .await
                .context(format!("post process of import: {:?}", i))?;
            config.merge(temp_config);
        }
        Ok(config)
    }
}
