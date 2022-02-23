use std::sync::Arc;

use crate::{
    deserialize_config,
    storage::{FileStorage, FolderType, Storage},
};

use super::{importer::get_importer, select_map::SelectMap, ConfigExt, Import, ImportSource};
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

        Ok(stream! {
            loop {
                let config = inner.deserialize_config(&source).await?;
                let (config, import) = inner.unfold_import(config).await?;
                yield inner.unfold_config(config).await;
                inner.wait_source(&source, &import).await?;
            }
        })
    }
    pub fn select_storage(&self) -> &dyn Storage {
        &self.inner.select_storage
    }
}

impl Import {
    async fn apply(&self, config: &mut Config, cache: &dyn Storage) -> Result<()> {
        let mut importer = get_importer(self)?;
        let content = self.source.get_content(cache).await?;
        importer.process(config, &content, cache).await?;
        Ok(())
    }
}

impl Inner {
    async fn deserialize_config(&self, source: &ImportSource) -> Result<ConfigExt> {
        let mut config = deserialize_config(&source.get_content(&self.file_cache).await?)?;
        config.config.id = source.cache_key();
        Ok(config)
    }
    async fn unfold_import(&self, mut config: ConfigExt) -> Result<(Config, Vec<Import>)> {
        let imports = config.import;

        for i in &imports {
            i.apply(&mut config.config, &self.file_cache)
                .await
                .context(format!("applying import: {:?}", i))?;
        }

        Ok((config.config, imports))
    }
    async fn unfold_config(&self, mut config: Config) -> Result<Config> {
        config.flatten_net(&self.delimiter, &self.registry)?;

        // restore patch
        SelectMap::from_cache(&config.id, &self.select_storage)
            .await?
            .apply_config(&mut config)
            .await;

        Ok(config)
    }
    async fn wait_source(&self, cfg_src: &ImportSource, imports: &[Import]) -> Result<()> {
        let mut events = FuturesUnordered::new();
        events.push(cfg_src.wait(&self.file_cache));
        for i in imports {
            events.push(i.source.wait(&self.file_cache));
        }
        events.next().await;
        Ok(())
    }
}
