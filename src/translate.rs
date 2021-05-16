mod clash;

use crate::config::Import;
use anyhow::{anyhow, Result};
use rabbit_digger::config::Config;
use tokio::fs::read_to_string;

#[cfg(not(feature = "translate"))]
pub async fn post_process(_config: &mut Config, _import: Import) -> Result<()> {
    Ok(())
}

#[cfg(feature = "translate")]
pub async fn post_process(config: &mut Config, import: Import) -> Result<()> {
    let content = read_to_string(import.path).await?;
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
