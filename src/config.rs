pub mod default;

use std::collections::BTreeMap;

use anyhow::Result;
use rd_interface::Value;
use serde_derive::{Deserialize, Serialize};

use crate::Registry;

pub type ConfigNet = BTreeMap<String, Net>;
pub type ConfigServer = BTreeMap<String, Server>;

#[derive(Debug)]
pub enum AllNet {
    Net(Net),
    Root(Vec<String>),
}

impl AllNet {
    pub fn get_dependency(&self, registry: &Registry) -> Result<Vec<String>> {
        Ok(match self {
            AllNet::Net(Net { net_type, opt }) => registry
                .get_net(net_type)?
                .resolver
                .get_dependency(opt.clone())?,
            AllNet::Root(v) => v.clone(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub net: ConfigNet,
    #[serde(default)]
    pub server: ConfigServer,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Net {
    #[serde(rename = "type")]
    pub net_type: String,
    #[serde(flatten)]
    pub opt: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Server {
    #[serde(rename = "type")]
    pub server_type: String,
    #[serde(default = "default::local_string")]
    pub listen: String,
    #[serde(default = "default::local_string")]
    pub net: String,
    #[serde(flatten)]
    pub opt: Value,
}

impl Config {
    pub fn merge(&mut self, other: Config) {
        self.net.extend(other.net);
        self.server.extend(other.server);
    }
}
