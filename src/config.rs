use anyhow::{Context, Result};
use rabbit_digger::Config;
use std::mem::replace;

pub async fn post_process(mut config: Config) -> Result<Config> {
    let imports = replace(&mut config.import, Vec::new());
    for i in imports {
        let mut temp_config = Config::default();
        crate::translate::post_process(&mut temp_config, i.clone())
            .await
            .context(format!("post process of import: {:?}", i))?;
        config.merge(temp_config);
    }
    Ok(config)
}
