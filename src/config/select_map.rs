use std::collections::HashMap;

use anyhow::Result;
use rabbit_digger::Config;
use serde::{Deserialize, Serialize};

use super::ConfigCache;

#[derive(Debug, Serialize, Deserialize)]
pub struct SelectMap(HashMap<String, String>);

impl SelectMap {
    pub async fn from_cache(id: &str, cache: &dyn ConfigCache) -> Result<SelectMap> {
        let select_map = cache
            .get(id)
            .await?
            .map(|i| serde_json::from_str(&i.content).unwrap_or_default())
            .unwrap_or_default();
        Ok(SelectMap(select_map))
    }
    pub async fn write_cache(&self, id: &str, cache: &dyn ConfigCache) -> Result<()> {
        cache.set(id, &serde_json::to_string(&self.0)?).await
    }
    pub async fn apply_config(&self, config: &mut Config) {
        for (net, selected) in &self.0 {
            if let Some(n) = config.net.get_mut(net) {
                if n.net_type == "select" {
                    if let Some(o) = n.opt.as_object_mut() {
                        o.insert("selected".to_string(), selected.to_string().into());
                    }
                }
            }
        }
    }
    pub fn insert(&mut self, key: String, value: String) -> Option<String> {
        self.0.insert(key, value)
    }
}
