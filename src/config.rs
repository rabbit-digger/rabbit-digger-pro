pub use self::{manager::ConfigManager, select_map::SelectMap};
use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use notify_stream::{notify::RecursiveMode, notify_stream};
use rabbit_digger::{Config, Registry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    future::pending,
    iter::once,
    mem::replace,
    path::PathBuf,
    time::{Duration, SystemTime},
};
use tokio::{fs::read_to_string, time::sleep};

use crate::{
    storage::{FileStorage, FolderType, Storage},
    util::DebounceStreamExt,
};

mod manager;
mod select_map;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportUrl {
    pub url: String,
    pub interval: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportStorage {
    pub folder: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ImportSource {
    Path(PathBuf),
    Poll(ImportUrl),
    Storage(ImportStorage),
}

impl ImportSource {
    pub fn cache_key(&self) -> String {
        match self {
            ImportSource::Path(path) => format!("path:{:?}", path),
            ImportSource::Poll(url) => format!("poll:{}", url.url),
            ImportSource::Storage(storage) => format!("storage:{}:{}", storage.folder, storage.key),
        }
    }
    pub async fn get_content(&self, cache: &dyn Storage) -> Result<String> {
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
            ImportSource::Poll(ImportUrl { url, .. }) => {
                tracing::info!("Fetching {}", url);
                let content = reqwest::get(url).await?.text().await?;
                tracing::info!("Done");
                cache.set(&key, &content).await?;
                content
            }
            ImportSource::Storage(ImportStorage { folder, key }) => {
                let storage = FileStorage::new(FolderType::Data, folder).await?;
                let item = storage
                    .get(&key)
                    .await?
                    .ok_or_else(|| anyhow!("Not found"))?;
                item.content
            }
        })
    }
    fn get_expire_duration(&self) -> Option<Duration> {
        match self {
            ImportSource::Path(_) => None,
            ImportSource::Poll(ImportUrl { interval, .. }) => {
                interval.map(|i| Duration::from_secs(i))
            }
            ImportSource::Storage(_) => None,
        }
    }
    pub async fn wait(&self, cache: &dyn Storage) -> Result<()> {
        match self {
            ImportSource::Path(path) => {
                let mut stream = notify_stream(path, RecursiveMode::NonRecursive)?
                    .debounce(Duration::from_millis(100));
                stream.next().await;
            }
            ImportSource::Poll(ImportUrl { interval, .. }) => {
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
            ImportSource::Storage(ImportStorage { folder, key }) => {
                let storage = FileStorage::new(FolderType::Data, folder).await?;
                let path = storage
                    .get_path(&key)
                    .await?
                    .ok_or_else(|| anyhow!("Not found"))?;

                let mut stream = notify_stream(path, RecursiveMode::NonRecursive)?
                    .debounce(Duration::from_millis(100));
                stream.next().await;
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

fn with_prefix(prefix: &str, v: Vec<String>) -> Vec<String> {
    once(prefix.to_string()).chain(v).collect()
}

impl ConfigExt {
    // Flatten nested net
    pub fn flatten_net(&mut self, delimiter: &str, registry: &Registry) -> Result<()> {
        loop {
            let mut to_add = HashMap::new();
            for (name, net) in self.config.net.iter_mut() {
                let path = registry
                    .get_net(&net.net_type)?
                    .resolver
                    .collect_net_ref(name, net.opt.clone())?
                    .into_iter()
                    .map(|(k, v)| (with_prefix("net", k), v));
                to_add.extend(path);
            }
            for (name, server) in self.config.server.iter_mut() {
                let path = registry
                    .get_server(&server.server_type)?
                    .resolver
                    .collect_net_ref(name, server.opt.clone())?
                    .into_iter()
                    .map(|(k, v)| (with_prefix("server", k), v));
                to_add.extend(path);
            }
            if to_add.len() == 0 {
                break;
            }

            let mut cfg = serde_json::to_value(replace(&mut self.config, Default::default()))?;
            let mut to_add_net = HashMap::<String, rabbit_digger::config::Net>::new();

            for (path, opt) in to_add.into_iter() {
                let key = path.join(delimiter);
                let pointer = format!("/{}", path.join("/"));

                match cfg.pointer_mut(&pointer) {
                    Some(val) => {
                        *val = Value::String(key.clone());
                        to_add_net.insert(key, serde_json::from_value(opt)?);
                    }
                    None => return Err(anyhow!("pointer not found: {}", pointer)),
                }
            }
            self.config = serde_json::from_value(cfg)?;

            for (key, value) in to_add_net {
                self.config.net.insert(key, value);
            }
        }

        Ok(())
    }
    pub async fn build_from_cache(self, cache: &dyn Storage) -> Result<Config> {
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
