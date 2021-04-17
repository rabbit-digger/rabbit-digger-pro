use crate::config::{local_chain, Config, Net, Server};
use anyhow::{anyhow, Result};
use serde_derive::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
struct ClashConfig {
    #[serde(rename = "socks-port")]
    socks_port: u16,
    proxies: Vec<Proxy>,
}

#[derive(Debug, Deserialize)]
struct Proxy {
    name: String,
    #[serde(rename = "type")]
    proxy_type: String,
    #[serde(flatten)]
    rest: Value,
}

fn proxy_to_net(p: Proxy) -> Result<Net> {
    let net = match p.proxy_type.as_ref() {
        "ss" => {
            #[derive(Debug, Deserialize)]
            struct Param {
                server: String,
                port: u16,
                cipher: String,
                password: String,
            }
            let params: Param = serde_json::from_value(p.rest)?;
            Net {
                name: p.name,
                net_type: "shadowsocks".to_string(),
                chain: local_chain(),
                rest: json!({
                    "server": params.server,
                    "port": params.port,
                    "cipher": params.cipher,
                    "password": params.password,
                }),
            }
        }
        _ => return Err(anyhow!("Unsupported proxy type: {}", p.proxy_type)),
    };
    Ok(net)
}

pub async fn process(config: &mut Config, content: String) -> Result<()> {
    let clash_config: ClashConfig = serde_yaml::from_str(&content)?;
    for p in clash_config.proxies {
        match proxy_to_net(p) {
            Ok(p) => config.net.push(p),
            Err(e) => log::warn!("proxy not translated: {:?}", e),
        };
    }
    config.server.push(Server {
        name: "clash_socks_port".to_string(),
        server_type: "socks5".to_string(),
        listen: "local".to_string(),
        net: "rule".to_string(),
        rest: json!({
            "address": "0.0.0.0",
            "port": clash_config.socks_port,
        }),
    });
    Ok(())
}
