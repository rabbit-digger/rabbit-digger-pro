use anyhow::Result;
use rabbit_digger::Config;
use serde::Deserialize;
use serde_json::{from_value, Value};

#[derive(Debug, Deserialize)]
pub struct Merge(Value);

pub fn from_config(value: Value) -> Result<Merge> {
    from_value(value).map_err(Into::into)
}

impl Merge {
    pub async fn process(&mut self, config: &mut Config, content: String) -> Result<()> {
        let other_content: Config = serde_json::from_str(&content)?;
        config.merge(other_content);
        Ok(())
    }
}
