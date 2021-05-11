use crate::Value;
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::HashMap, fmt::Debug, net::SocketAddr};
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
pub trait CommonField: DeserializeOwned + Serialize + 'static {
    const KEY: &'static str;
}

/// A context stores a source endpoint, a process info and other any values
/// during connecting.
#[derive(Debug, Clone)]
pub struct Context {
    data: HashMap<String, Value>,
    composite_list: Vec<String>,
}

impl Context {
    /// new a empty context
    pub fn new() -> Context {
        Context {
            data: HashMap::new(),
            composite_list: Vec::new(),
        }
    }
    /// new a context from socket addr
    pub fn from_socketaddr(addr: SocketAddr) -> Context {
        let mut ctx = Context::new();
        ctx.insert_common(common_field::SourceAddress { addr })
            .unwrap();
        ctx
    }
    /// Inserts a key-value pair into the context.
    pub fn insert<I: Serialize>(&mut self, key: String, value: I) -> Result<()> {
        self.data.insert(key, serde_json::to_value(value)?);
        Ok(())
    }
    /// Removes a key from the context
    pub fn remove<T: DeserializeOwned>(&mut self, key: &str) -> Result<()> {
        self.data.remove(key).ok_or(Error::NonExist)?;
        Ok(())
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
    pub fn remove_value(&mut self, key: &str) -> Result<()> {
        self.data.remove(key);
        Ok(())
    }
    /// Returns a value corresponding to the key.
    pub fn get_value(&self, key: &str) -> Result<Value> {
        match self.data.get(key) {
            Some(v) => Ok(v.to_owned()),
            None => Err(Error::NonExist),
        }
    }
    /// Inserts a key-value pair into the context.
    pub fn insert_common<T: CommonField>(&mut self, value: T) -> Result<()> {
        self.insert(T::KEY.to_string(), value)
    }
    /// Returns a value corresponding to the key.
    pub fn get_common<T: CommonField>(&self) -> Result<T> {
        self.get(T::KEY)
    }
    /// Add composite to composite_list
    pub fn append_composite(&mut self, composite_name: impl Into<String>) {
        self.composite_list.push(composite_name.into())
    }
    /// Get composite_list
    pub fn composite_list(&self) -> &Vec<String> {
        &self.composite_list
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
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct ProcessInfo {
        pub process_name: String,
    }

    impl CommonField for ProcessInfo {
        const KEY: &'static str = "process_info";
    }
}
