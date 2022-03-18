use anyhow::{anyhow, Result};
use rabbit_digger::Config;
use rd_interface::{
    async_trait, config::EmptyConfig, prelude::*, rd_config, registry::Builder, IntoDyn,
};
use rhai::{
    serde::{from_dynamic, to_dynamic},
    Engine, Scope,
};

use crate::storage::Storage;

use super::{BoxImporter, Importer};

#[rd_config]
#[derive(Debug)]
pub struct Rhai {}

#[async_trait]
impl Importer for Rhai {
    async fn process(
        &mut self,
        config: &mut Config,
        content: &str,
        _cache: &dyn Storage,
    ) -> Result<()> {
        let engine = Engine::new();
        let mut scope = Scope::new();
        let dyn_config = to_dynamic(&config).map_err(|e| anyhow!("to_dynamic err: {:?}", e))?;
        scope.push("config", dyn_config);

        engine
            .eval_with_scope(&mut scope, content)
            .map_err(|e| anyhow!("Failed to evaluate rhai: {:?}", e))?;

        if let Some(cfg) = scope.get_value("config") {
            *config = from_dynamic(&cfg).map_err(|e| anyhow!("from_dynamic err: {:?}", e))?;
        } else {
            return Err(anyhow!("Failed to get config from rhai"));
        }

        Ok(())
    }
}

impl Builder<BoxImporter> for Rhai {
    const NAME: &'static str = "rhai";

    type Config = EmptyConfig;
    type Item = Rhai;

    fn build(_cfg: Self::Config) -> rd_interface::Result<Self::Item> {
        Ok(Rhai {})
    }
}

impl IntoDyn<BoxImporter> for Rhai {
    fn into_dyn(self) -> BoxImporter {
        Box::new(self)
    }
}
