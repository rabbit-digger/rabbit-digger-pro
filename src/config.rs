pub mod default;

use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use rd_interface::Value;
use serde_derive::{Deserialize, Serialize};
use serde_with::{serde_as, OneOrMany};

pub type ConfigNet = HashMap<String, Net>;
pub type ConfigServer = HashMap<String, Server>;
pub type ConfigComposite = HashMap<String, CompositeName>;

#[derive(Debug)]
pub enum AllNet {
    Net(Net),
    Composite(CompositeName),
    Local,
    Noop,
    Root(Vec<String>),
}

impl AllNet {
    pub fn get_dependency(&self) -> Vec<&String> {
        match self {
            AllNet::Net(Net { chain, .. }) => chain.iter().collect(),
            AllNet::Composite(CompositeName {
                composite,
                net_list,
                ..
            }) => match &composite.0 {
                Composite::Rule(CompositeRule { rule }) => rule.iter().map(|i| &i.target).collect(),
                Composite::Select => net_list.iter().collect(),
            },
            AllNet::Root(v) => v.iter().collect(),
            _ => Vec::new(),
        }
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
#[serde(untagged)]
pub enum Chain {
    One(String),
    Many(Vec<String>),
}

impl Chain {
    pub fn into_vec(self) -> Vec<String> {
        match self {
            Chain::One(s) => vec![s],
            Chain::Many(v) => v,
        }
    }
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            Chain::One(s) => vec![s.clone()],
            Chain::Many(v) => v.clone(),
        }
    }
    pub fn as_ref(&self) -> Vec<&str> {
        match self {
            Chain::One(s) => vec![&s],
            Chain::Many(v) => v.iter().map(AsRef::as_ref).collect(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Net {
    #[serde(rename = "type")]
    pub net_type: String,
    #[serde_as(deserialize_as = "OneOrMany<_>")]
    #[serde(default = "default::local_chain")]
    pub chain: Vec<String>,
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
