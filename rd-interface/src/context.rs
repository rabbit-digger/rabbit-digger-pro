use crate::Value;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Map;
use std::{collections::HashMap, fmt, iter::FromIterator, mem::replace, net::SocketAddr};
use thiserror::Error;

/// Context error
#[derive(Debug, Error)]
pub enum Error {
    #[error("serde error {0}")]
    Serde(#[from] serde_json::Error),
    #[error("item not exists")]
    NonExist,
    #[error("Bad format")]
    BadFormat,
}
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Defines common used field with its key and type
pub trait CommonField: DeserializeOwned + Serialize + 'static {
    const KEY: &'static str;
}

/// A context stores a source endpoint, a process info and other any values
/// during connecting.
#[derive(Clone)]
pub struct Context {
    data: HashMap<String, Value>,
    net_list: Vec<String>,
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("Context");
        for (key, value) in &self.data {
            s.field(key, value);
        }
        s.finish()
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    /// new a empty context
    pub fn new() -> Context {
        Context {
            data: HashMap::new(),
            net_list: Vec::new(),
        }
    }
    /// new a context from socket addr
    pub fn from_socketaddr(addr: SocketAddr) -> Context {
        let mut ctx = Context::new();
        ctx.insert_common(common_field::SrcSocketAddr(addr))
            .expect("Failed to insert src_socket_addr");
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
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let value = self.data.get(key);
        value
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(Into::into)
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
    pub fn get_common<T: CommonField>(&self) -> Result<Option<T>> {
        self.get(T::KEY)
    }
    /// Add net to net_list
    pub fn append_net(&mut self, net_name: impl Into<String>) {
        self.net_list.push(net_name.into())
    }
    /// Get net_list
    pub fn net_list(&self) -> &Vec<String> {
        &self.net_list
    }
    /// Take net_list
    pub fn take_net_list(&mut self) -> Vec<String> {
        replace(&mut self.net_list, Vec::new())
    }
    /// to Value
    pub fn to_value(&self) -> Value {
        let mut map = Map::from_iter(self.data.clone().into_iter());
        map.insert(
            "net_list".to_string(),
            serde_json::to_value(&self.net_list).unwrap(),
        );
        Value::Object(map)
    }
    /// from Value
    pub fn from_value(value: Value) -> Result<Self> {
        let mut ctx = Context::new();
        if let Value::Object(mut map) = value {
            if let Some(net_list) = map.remove("net_list") {
                ctx.net_list = serde_json::from_value(net_list)?;
            }
            ctx.data = HashMap::from_iter(map.into_iter());
            Ok(ctx)
        } else {
            Err(Error::BadFormat)
        }
    }
}

/// Common context keys and types
pub mod common_field {
    use crate::address::AddressDomain;

    use super::CommonField;
    use serde::{Deserialize, Serialize};
    use std::net::SocketAddr;

    #[derive(Debug, Deserialize, Serialize)]
    pub struct ProcessInfo {
        pub process_name: String,
    }

    impl CommonField for ProcessInfo {
        const KEY: &'static str = "process_info";
    }

    /// format: `domain:port`
    #[derive(Debug, Deserialize, Serialize)]
    pub struct DestDomain(pub AddressDomain);

    impl CommonField for DestDomain {
        const KEY: &'static str = "dest_domain";
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct DestSocketAddr(pub SocketAddr);

    impl CommonField for DestSocketAddr {
        const KEY: &'static str = "dest_socket_addr";
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct SrcSocketAddr(pub SocketAddr);

    impl CommonField for SrcSocketAddr {
        const KEY: &'static str = "src_socket_addr";
    }
}
