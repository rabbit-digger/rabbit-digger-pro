use crate::config::{ConfigCache, Import};
use anyhow::{anyhow, Result};
use rabbit_digger::config::Config;

mod clash;

pub async fn post_process(
    config: &mut Config,
    import: Import,
    cache: &dyn ConfigCache,
) -> Result<()> {
    let content = import.source.get_content(cache).await?;
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
