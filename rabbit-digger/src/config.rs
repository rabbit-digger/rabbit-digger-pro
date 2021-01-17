use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    proxies: Vec<Proxy>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Proxy {
    name: String,
    #[serde(rename = "type")]
    proxy_type: String,
    server: String,
    port: u16,
    cipher: String,
    password: String,
    plugin: String,
    #[serde(rename = "plugin-opts")]
    plugin_opts: HashMap<String, String>,
}
