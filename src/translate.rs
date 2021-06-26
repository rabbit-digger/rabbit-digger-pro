mod clash;

use crate::config::{Import, ImportSource};
use anyhow::{anyhow, Result};
use rabbit_digger::config::Config;
use tokio::fs::read_to_string;

pub async fn get_source(source: &ImportSource) -> Result<String> {
    match source {
        ImportSource::Path(path) => Ok(read_to_string(path).await?),
    }
}

pub async fn post_process(config: &mut Config, import: Import) -> Result<()> {
    let content = get_source(&import.source).await?;
    match import.format.as_ref() {
        "clash" => {
            clash::from_config(import.opt)?
                .process(config, content)
                .await?
        }
        _ => return Err(anyhow!("format {} is not supported", import.format)),
    };
    Ok(())
}
