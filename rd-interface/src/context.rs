use crate::config::Value;
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::HashMap, fmt::Debug};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("serde error {0}")]
    Serde(#[from] serde_json::Error),
    #[error("item not exists")]
    NonExist,
}
pub type Result<T, E = Error> = std::result::Result<T, E>;

pub trait CommonField {
    const KEY: &'static str;
    type Type: DeserializeOwned + Serialize;
}

#[derive(Debug)]
pub struct Context {
    parent: Value,
    data: HashMap<String, Value>,
}

impl Context {
    pub fn new() -> Context {
        Context {
            parent: Value::Null,
            data: HashMap::new(),
        }
    }
    pub fn insert<I: Serialize>(&mut self, key: String, value: I) -> Result<()> {
        self.data.insert(key, serde_json::to_value(value)?);
        Ok(())
    }
    pub fn remove<T: DeserializeOwned>(&mut self, key: &str) -> Result<T> {
        let value = self.data.remove(key).ok_or(Error::NonExist)?;
        Ok(serde_json::from_value(value)?)
    }
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let value = self.data.get(key).ok_or(Error::NonExist)?;
        Ok(serde_json::from_value(value.clone())?)
    }
    pub fn insert_value(&mut self, key: String, value: Value) {
        self.data.insert(key, value);
    }
    pub fn remove_value(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }
    pub fn get_value(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }
    pub fn get_common<T: CommonField>(&self) -> Result<T::Type> {
        self.get(T::KEY)
    }
    pub fn insert_common<T: CommonField>(&mut self, value: T::Type) -> Result<()> {
        self.insert(T::KEY.to_string(), value)
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
