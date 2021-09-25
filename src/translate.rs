use crate::{config::Import, storage::Storage};
use anyhow::{anyhow, Result};
use rabbit_digger::config::Config;

mod clash;
mod merge;

pub async fn post_process(config: &mut Config, import: Import, cache: &dyn Storage) -> Result<()> {
    let content = import.source.get_content(cache).await?;
    match import.format.as_ref() {
        "clash" => {
            clash::from_config(import.opt)?
                .process(config, content)
                .await?
        }
        "merge" => {
            merge::from_config(import.opt)?
                .process(config, content)
                .await?
        }
        _ => return Err(anyhow!("format {} is not supported", import.format)),
    };
    Ok(())
}
