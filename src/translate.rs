mod clash;

use anyhow::{anyhow, Result};
use async_std::fs::read_to_string;

use rabbit_digger::config::{Config, Import};

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
