pub(crate) mod default;

use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use rd_interface::config::Value;
use serde_derive::{Deserialize, Serialize};

pub type ConfigNet = HashMap<String, Net>;
pub type ConfigServer = HashMap<String, Server>;
pub type ConfigComposite = HashMap<String, CompositeName>;

#[derive(Debug)]
pub enum AllNet {
    Net(Net),
    Composite(CompositeName),
    Local,
    Noop,
}

impl AllNet {
    pub fn get_dependency(&self) -> Vec<&String> {
        match self {
            AllNet::Net(Net { chain, .. }) => match chain {
                Chain::One(s) => vec![s],
                Chain::Many(v) => v.iter().collect(),
            },
            AllNet::Composite(CompositeName {
                composite,
                net_list,
                ..
            }) => match &composite.0 {
                Composite::Rule(CompositeRule { rule }) => rule.iter().map(|i| &i.target).collect(),
                Composite::Select => net_list
                    .0
                    .as_ref()
                    .map(|i| i.iter().collect())
                    .unwrap_or_default(),
            },
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default::plugins")]
    pub plugin_path: PathBuf,
    #[serde(default)]
    pub net: ConfigNet,
    #[serde(default)]
    pub server: ConfigServer,
    #[serde(default)]
    pub composite: ConfigComposite,
    pub import: Option<Vec<Import>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Import {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub format: String,
    pub path: PathBuf,
    #[serde(flatten)]
    pub rest: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NetList(pub Option<Vec<String>>);

/// Define a net composited from many other net
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompositeName {
    pub name: Option<String>,
    #[serde(default)]
    pub net_list: NetList,
    #[serde(flatten)]
    pub composite: CompositeDefaultType,
}

impl NetList {
    pub fn into_net_list(self) -> Result<Vec<String>> {
        self.0.ok_or(anyhow!("net_list is required"))
    }

    pub fn clone_net_list(&self) -> Result<Vec<String>> {
        self.0.clone().ok_or(anyhow!("net_list is required"))
    }

    pub fn as_ref(&self) -> Vec<&str> {
        self.0.iter().flatten().map(AsRef::as_ref).collect()
    }
}

impl From<Option<Vec<String>>> for NetList {
    fn from(i: Option<Vec<String>>) -> Self {
        NetList(i)
    }
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Net {
    #[serde(rename = "type")]
    pub net_type: String,
    #[serde(default = "default::local_chain")]
    pub chain: Chain,
    #[serde(flatten)]
    pub rest: Value,
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
    pub rest: Value,
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
    pub async fn post_process(mut self) -> Result<Self> {
        if let Some(imports) = (&mut self).import.take() {
            for i in imports {
                crate::translate::post_process(&mut self, i.clone())
                    .await
                    .context(format!("post process of import: {:?}", i))?;
            }
        }
        Ok(self)
    }
}
