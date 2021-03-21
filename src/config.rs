use std::path::PathBuf;

use rd_interface::config::Value;
use serde_derive::{Deserialize, Serialize};

pub type ConfigNet = Vec<Net>;
pub type ConfigServer = Vec<Server>;
pub type ConfigRule = Vec<Rule>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "plugins")]
    pub plugin_path: PathBuf,
    pub net: ConfigNet,
    pub server: ConfigServer,
    pub rule: ConfigRule,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rule {
    #[serde(rename = "type")]
    pub rule_type: String,
    pub target: String,
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
    pub name: String,
    #[serde(rename = "type")]
    pub net_type: String,
    #[serde(default = "local_chain")]
    pub chain: Chain,
    #[serde(flatten)]
    pub rest: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Server {
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: String,
    #[serde(default = "local_string")]
    pub listen: String,
    #[serde(default = "rule")]
    pub net: String,
    #[serde(flatten)]
    pub rest: Value,
}

fn local_chain() -> Chain {
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
