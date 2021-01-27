use std::path::PathBuf;

use rd_interface::config::Value;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "plugins")]
    pub plugin_path: PathBuf,
    pub net: Vec<Net>,
    pub server: Vec<Server>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Net {
    pub name: String,
    #[serde(rename = "type")]
    pub net_type: String,
    #[serde(default = "local")]
    pub chain: String,
    #[serde(flatten)]
    pub rest: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Server {
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: String,
    #[serde(default = "local")]
    pub listen: String,
    #[serde(default = "rule")]
    pub net: String,
    #[serde(flatten)]
    pub rest: Value,
}

fn local() -> String {
    "local".to_string()
}

fn rule() -> String {
    "rule".to_string()
}

fn plugins() -> PathBuf {
    PathBuf::from("plugins")
}
