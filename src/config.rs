use anyhow::{Context, Result};
use rabbit_digger::Config;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Import {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub format: String,
    pub path: PathBuf,
    #[serde(flatten)]
    pub opt: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigExt {
    #[serde(flatten)]
    config: Config,
    #[serde(default)]
    import: Vec<Import>,
}

impl ConfigExt {
    pub async fn post_process(self) -> Result<Config> {
        let imports = self.import;
        let mut config = self.config;
        for i in imports {
            let mut temp_config = Config::default();
            crate::translate::post_process(&mut temp_config, i.clone())
                .await
                .context(format!("post process of import: {:?}", i))?;
            config.merge(temp_config);
        }
        Ok(config)
    }
}
