use anyhow::Result;
use rabbit_digger::Config;
use rd_interface::async_trait;
use serde::Deserialize;
use serde_json::Value;

use crate::storage::Storage;

use super::Importer;

#[derive(Debug, Deserialize)]
pub struct Merge(Value);

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
