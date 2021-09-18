use std::sync::Arc;

use crate::deserialize_config;

use super::{
    cache::{CacheItem, FileCache},
    ConfigCache, ConfigExt, Import, ImportSource,
};
use anyhow::{Context, Result};
use async_stream::stream;
use futures::{stream::FuturesUnordered, Stream, StreamExt};
use rabbit_digger::Config;

const CFG_MGR_PREFIX: &'static str = "cfg_mgr.";
const SELECT_PREFIX: &'static str = "select.";

struct Inner {
    file_cache: FileCache,
    select_storage: FileCache,
}

#[derive(Clone)]
pub struct ConfigManager {
    inner: Arc<Inner>,
}

impl ConfigManager {
    pub async fn new() -> Result<Self> {
        let file_cache = FileCache::new(CFG_MGR_PREFIX).await?;
        let select_storage = FileCache::new(SELECT_PREFIX).await?;

        let mgr = ConfigManager {
            inner: Arc::new(Inner {
                file_cache,
                select_storage,
            }),
        };

        Ok(mgr)
    }
    pub async fn config_stream(
        &self,
        source: ImportSource,
    ) -> Result<impl Stream<Item = Result<Config>>> {
        let inner = self.inner.clone();
        let mut config = inner.deserialize_config(&source).await?;

        Ok(stream! {
            loop {
                yield inner.get_config(&config).await;
                inner.wait_source(&source, &config.import).await?;
                config = inner.deserialize_config(&source).await?;
            }
        })
    }
    pub fn select_storage(&self) -> &dyn ConfigCache {
        &self.inner.select_storage
    }
}

impl Inner {
    async fn deserialize_config(&self, source: &ImportSource) -> Result<ConfigExt> {
        let mut config = deserialize_config(&source.get_content(&self.file_cache).await?)?;
        config.config.id = source.cache_key();
        Ok(config)
    }
    async fn get_config(&self, config: &ConfigExt) -> Result<Config> {
        let mut config = config.clone();

        let imports = config.import;
        for i in imports {
            crate::translate::post_process(&mut config.config, i.clone(), &self.file_cache)
                .await
                .context(format!("post process of import: {:?}", i))?;
        }

        // restore patch
        let patch = self.select_storage.get(&config.config.id).await;
        if let Ok(Some(CacheItem { content, .. })) = patch {
            let mut config_value = serde_json::to_value(&config.config)?;
            json_patch::patch(&mut config_value, &serde_json::from_str(&content)?)?;
            config.config = serde_json::from_value(config_value)?;
        }

        Ok(config.config)
    }
    async fn wait_source(&self, cfg_src: &ImportSource, imports: &Vec<Import>) -> Result<()> {
        let mut events = FuturesUnordered::new();
        events.push(cfg_src.wait(&self.file_cache));
        for Import { source, .. } in imports {
            events.push(source.wait(&self.file_cache));
        }
        events.next().await;
        Ok(())
    }
}
