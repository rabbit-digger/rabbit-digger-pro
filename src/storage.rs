use std::time::SystemTime;

use anyhow::Result;
use rd_interface::async_trait;
use serde::{Deserialize, Serialize};

pub use self::{
    file::{FileStorage, FolderType},
    memory::MemoryCache,
};

mod file;
mod memory;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageItem {
    pub updated_at: SystemTime,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageKey {
    pub updated_at: SystemTime,
    pub key: String,
}

#[async_trait]
pub trait Storage: Send + Sync {
    async fn get_updated_at(&self, key: &str) -> Result<Option<SystemTime>>;
    async fn get(&self, key: &str) -> Result<Option<StorageItem>>;
    async fn set(&self, key: &str, value: &str) -> Result<()>;
    async fn remove(&self, key: &str) -> Result<()>;
    async fn keys(&self) -> Result<Vec<StorageKey>>;
    async fn clear(&self) -> Result<()>;
}

// all key-values in `from` will be copied to `to`
pub async fn assign_storage(from: &impl Storage, to: &impl Storage) -> Result<()> {
    let keys = from.keys().await?;

    for key in keys {
        let to_updated_at = to
            .get_updated_at(&key.key)
            .await?
            .unwrap_or_else(|| SystemTime::UNIX_EPOCH);

        if to_updated_at >= key.updated_at {
            continue;
        }

        if let Some(item) = from.get(&key.key).await? {
            to.set(&key.key, &item.content).await?;
        }
    }

    Ok(())
}
