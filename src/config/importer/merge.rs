use anyhow::Result;
use rabbit_digger::Config;
use rd_interface::{async_trait, config::EmptyConfig, registry::Builder, IntoDyn};

use crate::storage::Storage;

use super::{BoxImporter, Importer};

#[derive(Debug)]
pub struct Merge;

#[async_trait]
impl Importer for Merge {
    async fn process(
        &mut self,
        config: &mut Config,
        content: &str,
        _cache: &dyn Storage,
    ) -> Result<()> {
        let other_content: Config = serde_yaml::from_str(content)?;
        config.merge(other_content);
        Ok(())
    }
}

impl Builder<BoxImporter> for Merge {
    const NAME: &'static str = "merge";

    type Config = EmptyConfig;

    type Item = Merge;

    fn build(_config: Self::Config) -> rd_interface::Result<Self::Item> {
        Ok(Merge)
    }
}

impl IntoDyn<BoxImporter> for Merge {
    fn into_dyn(self) -> BoxImporter {
        Box::new(self)
    }
}
