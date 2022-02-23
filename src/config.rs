pub use self::{importer::get_importer_registry, manager::ConfigManager, select_map::SelectMap};
use anyhow::{anyhow, Result};
use futures::StreamExt;
use notify_stream::{notify::RecursiveMode, notify_stream};
use rabbit_digger::Config;
use rd_interface::{
    prelude::*,
    rd_config,
    schemars::{schema::SchemaObject, schema_for},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    future::pending,
    path::PathBuf,
    time::{Duration, SystemTime},
};
use tokio::{fs::read_to_string, time::sleep};

use crate::{
    storage::{FileStorage, FolderType, Storage},
    util::DebounceStreamExt,
};

mod importer;
mod manager;
mod select_map;

#[rd_config]
#[derive(Debug, Clone)]
pub struct ImportUrl {
    pub url: String,
    pub interval: Option<u64>,
}

#[rd_config]
#[derive(Debug, Clone)]
pub struct ImportStorage {
    pub folder: String,
    pub key: String,
}

#[rd_config]
#[derive(Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ImportSource {
    Path(PathBuf),
    Poll(ImportUrl),
    Storage(ImportStorage),
}

impl ImportSource {
    pub fn new_path(path: PathBuf) -> Self {
        ImportSource::Path(path)
    }
    pub fn new_poll(url: String, interval: Option<u64>) -> Self {
        ImportSource::Poll(ImportUrl { url, interval })
    }
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
                    .get(key)
                    .await?
                    .ok_or_else(|| anyhow!("Not found"))?;
                item.content
            }
        })
    }
    fn get_expire_duration(&self) -> Option<Duration> {
        match self {
            ImportSource::Path(_) => None,
            ImportSource::Poll(ImportUrl { interval, .. }) => interval.map(Duration::from_secs),
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
                            .unwrap_or(Duration::ZERO);
                        sleep(tts).await
                    }
                }
            }
            ImportSource::Storage(ImportStorage { folder, key }) => {
                let storage = FileStorage::new(FolderType::Data, folder).await?;
                let path = storage
                    .get_path(key)
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

#[rd_config]
#[derive(Debug, Clone)]
pub struct Import {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub format: String,
    pub(super) source: ImportSource,
    #[serde(flatten)]
    pub opt: Value,
}

impl Import {
    // Append fields other than opt to a schema
    pub(crate) fn append_schema(mut schema: SchemaObject) -> SchemaObject {
        let properties = &mut schema.object().properties;
        properties.insert(
            "name".to_string(),
            schema_for!(Option<String>).schema.into(),
        );
        properties.insert(
            "source".to_string(),
            schema_for!(ImportSource).schema.into(),
        );
        schema.object().required.insert("source".to_string());
        schema
    }
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
