pub mod default;

use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use rd_interface::Value;
use serde_derive::{Deserialize, Serialize};

use crate::Registry;

pub type ConfigNet = HashMap<String, Net>;
pub type ConfigServer = HashMap<String, Server>;
pub type ConfigComposite = HashMap<String, CompositeName>;

#[derive(Debug)]
pub enum AllNet {
    Net(Net),
    Composite(CompositeName),
    Root(Vec<String>),
}

impl AllNet {
    pub fn get_dependency(&self, registry: &Registry) -> Result<Vec<String>> {
        Ok(match self {
            AllNet::Net(Net { net_type, opt }) => registry
                .get_net(net_type)?
                .resolver
                .get_dependency(opt.clone())?,
            AllNet::Composite(CompositeName {
                composite,
                net_list,
                ..
            }) => match &composite.0 {
                Composite::Rule(CompositeRule { rule }) => {
                    rule.iter().map(|i| i.target.clone()).collect()
                }
                Composite::Select => net_list.clone(),
            },
            AllNet::Root(v) => v.clone(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default::plugins")]
    pub plugin_path: PathBuf,
    #[serde(default)]
    pub net: ConfigNet,
    #[serde(default)]
    pub server: ConfigServer,
    #[serde(default)]
    pub composite: ConfigComposite,
    #[serde(default)]
    pub import: Vec<Import>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Import {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub format: String,
    pub path: PathBuf,
    #[serde(flatten)]
    pub opt: Value,
}

/// Define a net composited from many other net
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompositeName {
    pub name: Option<String>,
    #[serde(default)]
    pub net_list: Vec<String>,
    #[serde(flatten)]
    pub composite: CompositeDefaultType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Composite {
    Rule(CompositeRule),
    Select,
}

impl Into<CompositeDefaultType> for Composite {
    fn into(self) -> CompositeDefaultType {
        CompositeDefaultType(self)
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct CompositeDefaultType(pub Composite);

impl<'de> serde::Deserialize<'de> for CompositeDefaultType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        let v = Value::deserialize(deserializer)?;
        match Option::<String>::deserialize(&v["type"]).map_err(de::Error::custom)? {
            Some(_) => {
                let inner = Composite::deserialize(v).map_err(de::Error::custom)?;
                Ok(CompositeDefaultType(inner))
            }
            None => {
                let inner = CompositeRule::deserialize(v).map_err(de::Error::custom)?;
                Ok(CompositeDefaultType(Composite::Rule(inner)))
            }
        }
    }
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
    #[serde(default = "default::rule")]
    pub net: String,
    #[serde(flatten)]
    pub opt: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompositeRuleItem {
    pub target: String,
    #[serde(flatten)]
    pub matcher: Matcher,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompositeRule {
    pub rule: Vec<CompositeRuleItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Matcher {
    Domain { method: String, domain: String },
    IpCidr { ip_cidr: String },
    Any,
}

impl Config {
    pub fn merge(&mut self, other: Config) {
        self.plugin_path = other.plugin_path;
        self.net.extend(other.net);
        self.server.extend(other.server);
        self.composite.extend(other.composite);
        self.import.extend(other.import);
    }
}
