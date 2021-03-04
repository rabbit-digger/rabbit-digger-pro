use crate::config::Value;
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::HashMap, fmt::Debug};
use thiserror::Error;

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

/// A context stores a source endpoint, a process info and other any values
/// during connecting.
#[derive(Debug)]
pub struct Context {
    parent: Value,
    data: HashMap<String, Value>,
}

impl Context {
    /// new a empty context
    pub fn new() -> Context {
        Context {
            parent: Value::Null,
            data: HashMap::new(),
        }
    }
    /// Inserts a key-value pair into the context.
    pub fn insert<I: Serialize>(&mut self, key: String, value: I) -> Result<()> {
        self.data.insert(key, serde_json::to_value(value)?);
        Ok(())
    }
    /// Removes a key from the context, returning the value at the key if the key was previously in the context.
    pub fn remove<T: DeserializeOwned>(&mut self, key: &str) -> Result<T> {
        let value = self.data.remove(key).ok_or(Error::NonExist)?;
        Ok(serde_json::from_value(value)?)
    }
    /// Returns a value corresponding to the key.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let value = self.data.get(key).ok_or(Error::NonExist)?;
        Ok(serde_json::from_value(value.clone())?)
    }
    /// Inserts a key-value pair into the context.
    pub fn insert_value(&mut self, key: String, value: Value) {
        self.data.insert(key, value);
    }
    /// Removes a key from the context, returning the value at the key if the key was previously in the context.
    pub fn remove_value(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }
    /// Returns a value corresponding to the key.
    pub fn get_value(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }
    /// Inserts a key-value pair into the context.
    pub fn insert_common<T: CommonField>(&mut self, value: T::Type) -> Result<()> {
        self.insert(T::KEY.to_string(), value)
    }
    /// Returns a value corresponding to the key.
    pub fn get_common<T: CommonField>(&self) -> Result<T::Type> {
        self.get(T::KEY)
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
