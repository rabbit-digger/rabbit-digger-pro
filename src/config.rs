pub mod default;

use std::{borrow::Cow, collections::HashMap, iter::once, mem::replace};

use indexmap::IndexMap;
use rd_interface::{
    schemars::{self, JsonSchema},
    Result, Value,
};
use serde::{Deserialize, Serialize};

pub type ConfigNet = IndexMap<String, Net>;
pub type ConfigServer = IndexMap<String, Server>;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub net: ConfigNet,
    #[serde(default)]
    pub server: ConfigServer,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, JsonSchema)]
pub struct NetMetadata {
    /// Reset all connections passing through this Net
    reset_on_change: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Net {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub metadata: Option<NetMetadata>,
    #[serde(flatten)]
    pub _reserved: Reserved,
    #[serde(rename = "type")]
    pub net_type: String,
    #[serde(flatten)]
    pub opt: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ServerMetadata {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Server {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub metadata: Option<ServerMetadata>,
    #[serde(flatten)]
    pub _reserved: Reserved,
    #[serde(rename = "type")]
    pub server_type: String,
    #[serde(flatten)]
    pub opt: Value,
}

impl Config {
    pub fn merge(&mut self, other: Config) {
        self.net.extend(other.net);
        self.server.extend(other.server);
    }
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Reserved {
    #[serde(default, skip_serializing)]
    r#async: bool,
    #[serde(default, skip_serializing)]
    r#await: bool,
    #[serde(default, skip_serializing)]
    r#as: bool,
    #[serde(default, skip_serializing)]
    r#break: bool,
    #[serde(default, skip_serializing)]
    r#const: bool,
    #[serde(default, skip_serializing)]
    name: bool,
}

impl Net {
    pub fn new(net_type: impl Into<String>, opt: Value) -> Net {
        Net {
            net_type: net_type.into(),
            opt,
            _reserved: Default::default(),
            metadata: Default::default(),
        }
    }
    pub fn new_opt(
        net_type: impl Into<String>,
        opt: impl serde::Serialize,
    ) -> rd_interface::Result<Net> {
        Ok(Net::new(net_type, serde_json::to_value(opt)?))
    }
    pub fn metadata<'a>(&'a self) -> Cow<'a, NetMetadata> {
        match self.metadata.as_ref() {
            Some(m) => Cow::Borrowed(m),
            None => Cow::Owned(Default::default()),
        }
    }
}

impl Server {
    pub fn new(server_type: impl Into<String>, opt: Value) -> Server {
        Server {
            server_type: server_type.into(),
            opt,
            _reserved: Default::default(),
            metadata: Default::default(),
        }
    }
    pub fn new_opt(
        server_type: impl Into<String>,
        opt: impl serde::Serialize,
    ) -> rd_interface::Result<Server> {
        Ok(Server::new(server_type, serde_json::to_value(opt)?))
    }
    pub fn metadata<'a>(&'a self) -> Cow<'a, ServerMetadata> {
        match self.metadata.as_ref() {
            Some(m) => Cow::Borrowed(m),
            None => Cow::Owned(Default::default()),
        }
    }
}

fn with_prefix(prefix: &str, v: Vec<String>) -> Vec<String> {
    once(prefix.to_string()).chain(v).collect()
}

impl Config {
    // Flatten nested net
    pub fn flatten_net(&mut self, delimiter: &str, registry: &crate::Registry) -> Result<()> {
        loop {
            let mut to_add = HashMap::new();
            for (name, net) in self.net.iter_mut() {
                let path = registry
                    .get_net(&net.net_type)?
                    .resolver
                    .collect_net_ref(name, net.opt.clone())?
                    .into_iter()
                    .map(|(k, v)| (with_prefix("net", k), v));
                to_add.extend(path);
            }
            for (name, server) in self.server.iter_mut() {
                let path = registry
                    .get_server(&server.server_type)?
                    .resolver
                    .collect_net_ref(name, server.opt.clone())?
                    .into_iter()
                    .map(|(k, v)| (with_prefix("server", k), v));
                to_add.extend(path);
            }
            if to_add.len() == 0 {
                break;
            }

            let mut cfg = serde_json::to_value(replace(self, Default::default()))?;
            let mut to_add_net = HashMap::<String, Net>::new();

            for (path, opt) in to_add.into_iter() {
                let key = path.join(delimiter);
                let pointer = format!("/{}", path.join("/"));

                match cfg.pointer_mut(&pointer) {
                    Some(val) => {
                        *val = Value::String(key.clone());
                        to_add_net.insert(key, serde_json::from_value(opt)?);
                    }
                    None => {
                        return Err(rd_interface::Error::other(format!(
                            "pointer not found: {}",
                            pointer
                        )))
                    }
                }
            }
            *self = serde_json::from_value(cfg)?;

            for (key, value) in to_add_net {
                self.net.insert(key, value);
            }
        }

        Ok(())
    }
}
