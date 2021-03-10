use crate::config::Value;
use futures_util::future::BoxFuture;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Debug},
};
use thiserror::Error;

enum Lazy<T> {
    Value(T),
    Future(Box<dyn Fn() -> BoxFuture<'static, T> + Send + Sync>),
}

/// Context error
#[derive(Debug, Error)]
pub enum Error {
    #[error("serde error {0}")]
    Serde(#[from] serde_json::Error),
    #[error("item not exists")]
    NonExist,
}
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Defines common used field with its key and type
pub trait CommonField {
    const KEY: &'static str;
    type Type: DeserializeOwned + Serialize;
}

impl<T: Debug + Clone> fmt::Debug for Lazy<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Lazy::Value(value) => f.debug_tuple("Lazy").field(value).finish(),
            Lazy::Future(_) => f.debug_tuple("Lazy").finish(),
        }
    }
}

impl<T: Debug + Clone> Lazy<T> {
    async fn get(&self) -> T {
        match self {
            Lazy::Value(v) => v.clone(),
            Lazy::Future(future) => future().await,
        }
    }
}

/// A context stores a source endpoint, a process info and other any values
/// during connecting.
#[derive(Debug)]
pub struct Context {
    data: HashMap<String, Lazy<Value>>,
}

impl Context {
    /// new a empty context
    pub fn new() -> Context {
        Context {
            data: HashMap::new(),
        }
    }
    /// Inserts a key-value pair into the context. Value can be compute later.
    pub async fn insert_value_lazy(
        &mut self,
        key: String,
        f: impl Fn() -> BoxFuture<'static, Value> + Send + Sync + 'static,
    ) -> Result<()> {
        self.data.insert(key, Lazy::Future(Box::new(f)));
        Ok(())
    }
    /// Inserts a key-value pair into the context.
    pub async fn insert<I: Serialize>(&mut self, key: String, value: I) -> Result<()> {
        self.data
            .insert(key, Lazy::Value(serde_json::to_value(value)?));
        Ok(())
    }
    /// Removes a key from the context
    pub async fn remove<T: DeserializeOwned>(&mut self, key: &str) -> Result<()> {
        self.data.remove(key).ok_or(Error::NonExist)?;
        Ok(())
    }
    /// Returns a value corresponding to the key.
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let value = self.data.get(key).ok_or(Error::NonExist)?;
        Ok(serde_json::from_value(value.get().await)?)
    }
    /// Inserts a key-value pair into the context.
    pub async fn insert_value(&mut self, key: String, value: Value) {
        self.data.insert(key, Lazy::Value(value));
    }
    /// Removes a key from the context, returning the value at the key if the key was previously in the context.
    pub async fn remove_value(&mut self, key: &str) -> Option<Value> {
        match self.data.remove(key) {
            Some(v) => Some(v.get().await),
            None => None,
        }
    }
    /// Returns a value corresponding to the key.
    pub async fn get_value(&self, key: &str) -> Option<Value> {
        match self.data.get(key) {
            Some(v) => Some(v.get().await),
            None => None,
        }
    }
    /// Inserts a key-value pair into the context.
    pub async fn insert_common<T: CommonField>(&mut self, value: T::Type) -> Result<()> {
        self.insert(T::KEY.to_string(), value).await
    }
    /// Returns a value corresponding to the key.
    pub async fn get_common<T: CommonField>(&self) -> Result<T::Type> {
        self.get(T::KEY).await
    }
}

/// Common context keys and types
pub mod common_field {
    use super::CommonField;
    use serde_derive::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize)]
    pub struct SourceAddress {
        pub addr: std::net::SocketAddr,
    }

    impl CommonField for SourceAddress {
        const KEY: &'static str = "source_address";
        type Type = SourceAddress;
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct ProcessInfo {
        pub process_name: String,
    }

    impl CommonField for ProcessInfo {
        const KEY: &'static str = "process_info";
        type Type = ProcessInfo;
    }
}
