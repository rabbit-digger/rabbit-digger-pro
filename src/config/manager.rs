use std::sync::Arc;

use crate::{
    deserialize_config,
    storage::{FileStorage, FolderType, Storage},
};

use super::{select_map::SelectMap, ConfigExt, Import, ImportSource};
use anyhow::{Context, Result};
use async_stream::stream;
use futures::{stream::FuturesUnordered, Stream, StreamExt};
use rabbit_digger::{Config, Registry};

const CFG_MGR_PREFIX: &str = "cfg_mgr";
const SELECT_PREFIX: &str = "select";

struct Inner {
    file_cache: FileStorage,
    select_storage: FileStorage,
    registry: Registry,
    delimiter: String,
}

#[derive(Clone)]
pub struct ConfigManager {
    inner: Arc<Inner>,
}

impl ConfigManager {
    pub async fn new(registry: Registry) -> Result<Self> {
        let file_cache = FileStorage::new(FolderType::Cache, CFG_MGR_PREFIX).await?;
        let select_storage = FileStorage::new(FolderType::Data, SELECT_PREFIX).await?;

        let mgr = ConfigManager {
            inner: Arc::new(Inner {
                file_cache,
                select_storage,
                registry,
                delimiter: "##".to_string(),
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
    pub fn select_storage(&self) -> &dyn Storage {
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

        for i in &config.import {
            crate::translate::post_process(&mut config.config, i.clone(), &self.file_cache)
                .await
                .context(format!("post process of import: {:?}", i))?;
        }

        config.config.flatten_net(&self.delimiter, &self.registry)?;

        // restore patch
        SelectMap::from_cache(&config.config.id, &self.select_storage)
            .await?
            .apply_config(&mut config.config)
            .await;

        Ok(config.config)
    }
    async fn wait_source(&self, cfg_src: &ImportSource, imports: &[Import]) -> Result<()> {
        let mut events = FuturesUnordered::new();
        events.push(cfg_src.wait(&self.file_cache));
        for Import { source, .. } in imports {
            events.push(source.wait(&self.file_cache));
        }
        events.next().await;
        Ok(())
    }
}
