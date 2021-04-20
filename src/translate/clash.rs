use crate::config::{local_chain, Config, Net, Server};
use anyhow::{anyhow, Result};
use serde_derive::Deserialize;
use serde_json::{from_value, json, Value};

#[derive(Debug, Deserialize)]
pub struct Clash {
    prefix: Option<String>,
    target: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClashConfig {
    #[serde(rename = "socks-port")]
    socks_port: u16,
    proxies: Vec<Proxy>,
    #[serde(rename = "proxy-groups")]
    proxy_groups: Vec<ProxyGroup>,
}

#[derive(Debug, Deserialize)]
struct Proxy {
    name: String,
    #[serde(rename = "type")]
    proxy_type: String,
    #[serde(flatten)]
    rest: Value,
}

#[derive(Debug, Deserialize)]
struct ProxyGroup {
    name: String,
    #[serde(rename = "type")]
    proxy_group_type: String,
    proxies: Vec<String>,
}

pub fn from_config(value: Value) -> Result<Clash> {
    from_value(value).map_err(Into::into)
}

impl Clash {
    fn proxy_to_net(&self, p: Proxy) -> Result<(String, Net)> {
        let net = match p.proxy_type.as_ref() {
            "ss" => {
                #[derive(Debug, Deserialize)]
                struct Param {
                    server: String,
                    port: u16,
                    cipher: String,
                    password: String,
                    udp: Option<bool>,
                }
                let params: Param = serde_json::from_value(p.rest)?;
                (
                    p.name,
                    Net {
                        net_type: "shadowsocks".to_string(),
                        chain: local_chain(),
                        rest: json!({
                            "server": params.server,
                            "port": params.port,
                            "cipher": params.cipher,
                            "password": params.password,
                            "udp": params.udp.unwrap_or_default(),
                        }),
                    },
                )
            }
            _ => return Err(anyhow!("Unsupported proxy type: {}", p.proxy_type)),
        };
        Ok(net)
    }

    fn prefix(&self, s: impl AsRef<str>) -> String {
        match &self.prefix {
            Some(prefix) => format!("{}.{}", prefix, s.as_ref()),
            None => s.as_ref().to_string(),
        }
    }

    pub async fn process(&self, config: &mut Config, content: String) -> Result<()> {
        let clash_config: ClashConfig = serde_yaml::from_str(&content)?;
        for p in clash_config.proxies {
            match self.proxy_to_net(p) {
                Ok((name, p)) => {
                    config.net.insert(self.prefix(name), p);
                }
                Err(e) => log::warn!("proxy not translated: {:?}", e),
            };
        }
        if let Some(target) = &self.target {
            config.server.insert(
                self.prefix("socks_port"),
                Server {
                    server_type: "socks5".to_string(),
                    listen: "local".to_string(),
                    net: target.to_string(),
                    rest: json!({
                        "address": "0.0.0.0",
                        "port": clash_config.socks_port,
                    }),
                },
            );
        }

        Ok(())
    }
}
