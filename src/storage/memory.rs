use std::{collections::HashMap, time::SystemTime};

use super::{Storage, StorageItem};
use anyhow::Result;
use parking_lot::RwLock;
use rd_interface::async_trait;

pub struct MemoryCache {
    cache: RwLock<HashMap<String, StorageItem>>,
}

impl MemoryCache {
    #[allow(dead_code)]
    pub async fn new() -> Result<Self> {
        Ok(MemoryCache {
            cache: RwLock::new(HashMap::new()),
        })
    }
}

#[async_trait]
impl Storage for MemoryCache {
    async fn get_updated_at(&self, key: &str) -> Result<Option<SystemTime>> {
        Ok(self.cache.read().get(key).map(|item| item.updated_at))
    }
    async fn get(&self, key: &str) -> Result<Option<StorageItem>> {
        Ok(self.cache.read().get(key).cloned())
    }
    async fn set(&self, key: &str, value: &str) -> Result<()> {
        self.cache.write().insert(
            key.to_string(),
            StorageItem {
                updated_at: SystemTime::now(),
                content: value.to_string(),
            },
        );
        Ok(())
    }
    async fn keys(&self) -> Result<Vec<String>> {
        Ok(self.cache.read().keys().map(|i| i.clone()).collect())
    }
}
