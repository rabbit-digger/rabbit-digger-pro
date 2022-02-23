use std::collections::BTreeMap;

use crate::{config::Import, storage::Storage};
use anyhow::{anyhow, Result};
use once_cell::sync::OnceCell;
use rabbit_digger::config::Config;
use rd_interface::{async_trait, Value};

mod clash;
mod merge;

type Registry =
    BTreeMap<&'static str, Box<dyn Fn(Value) -> Result<Box<dyn Importer>> + Send + Sync>>;
pub fn get_importer_registry() -> &'static Registry {
    static REGISTRY: OnceCell<Registry> = OnceCell::new();
    REGISTRY.get_or_init(|| {
        let mut registry = BTreeMap::new();
        fn insert<T: Importer + serde::de::DeserializeOwned + 'static>(
            r: &mut Registry,
            name: &'static str,
        ) {
            r.insert(
                name,
                Box::new(|v| Ok(Box::new(serde_json::from_value::<T>(v)?))),
            );
        }
        insert::<clash::Clash>(&mut registry, "clash");
        insert::<merge::Merge>(&mut registry, "merge");

        registry
    })
}

pub fn get_importer(import: &Import) -> Result<Box<dyn Importer>> {
    let registry = get_importer_registry();
    let importer_builder = registry
        .get(&import.format.as_ref())
        .ok_or_else(|| anyhow!("Importer not found: {}", import.format))?;
    importer_builder(import.opt.clone())
}

#[async_trait]
pub trait Importer: Send + Sync {
    async fn process(
        &mut self,
        config: &mut Config,
        content: &str,
        cache: &dyn Storage,
    ) -> Result<()>;
}
