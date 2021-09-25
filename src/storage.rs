use std::time::SystemTime;

use anyhow::Result;
use rd_interface::async_trait;
use serde::{Deserialize, Serialize};

pub use self::{file::FileStorage, memory::MemoryCache};

mod file;
mod memory;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageItem {
    pub updated_at: SystemTime,
    pub content: String,
}

#[async_trait]
pub trait Storage: Send + Sync {
    async fn get_updated_at(&self, key: &str) -> Result<Option<SystemTime>>;
    async fn get(&self, key: &str) -> Result<Option<StorageItem>>;
    async fn set(&self, key: &str, value: &str) -> Result<()>;
    async fn keys(&self) -> Result<Vec<String>>;
}
