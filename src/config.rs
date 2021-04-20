use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result};
use rd_interface::config::Value;
use serde_derive::{Deserialize, Serialize};

pub type ConfigNet = HashMap<String, Net>;
pub type ConfigServer = HashMap<String, Server>;
pub type ConfigComposite = HashMap<String, Composite>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "plugins")]
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

/// Define a net composited from many other net
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Composite {
    pub name: Option<String>,
    #[serde(rename = "type", default = "rule")]
    pub composite_type: String,
    #[serde(flatten)]
    pub rest: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Chain {
    One(String),
    Many(Vec<String>),
}

impl Chain {
    pub fn to_vec(self) -> Vec<String> {
        match self {
            Chain::One(s) => vec![s],
            Chain::Many(v) => v,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Net {
    #[serde(rename = "type")]
    pub net_type: String,
    #[serde(default = "local_chain")]
    pub chain: Chain,
    #[serde(flatten)]
    pub rest: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Server {
    #[serde(rename = "type")]
    pub server_type: String,
    #[serde(default = "local_string")]
    pub listen: String,
    #[serde(default = "rule")]
    pub net: String,
    #[serde(flatten)]
    pub rest: Value,
}

pub(crate) fn local_chain() -> Chain {
    Chain::One("local".to_string())
}

fn local_string() -> String {
    "local".to_string()
}

fn rule() -> String {
    "rule".to_string()
}

fn plugins() -> PathBuf {
    PathBuf::from("plugins")
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
