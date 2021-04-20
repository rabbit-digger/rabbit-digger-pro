use std::collections::HashMap;

use crate::config::{local_chain, Composite, Config, ConfigRuleItem, Net, Server};
use anyhow::{anyhow, Result};
use serde_derive::Deserialize;
use serde_json::{from_value, json, Value};

#[derive(Debug, Deserialize)]
pub struct Clash {
    prefix: Option<String>,
    target: Option<String>,
    direct: Option<String>,
    // reverse map from clash name to net name
    #[serde(skip)]
    name_map: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ClashConfig {
    socks_port: u16,
    proxies: Vec<Proxy>,
    proxy_groups: Vec<ProxyGroup>,
    rules: Vec<String>,
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

    fn proxy_group_to_composite(&self, p: ProxyGroup) -> Result<(String, Composite)> {
        Ok(match p.proxy_group_type.as_ref() {
            "select" => (
                self.proxy_group_name(&p.name),
                Composite {
                    name: Some(p.name),
                    composite_type: "rule".to_string(),
                    rest: json!({
                        "rule": []
                    }),
                },
            ),
            _ => {
                return Err(anyhow!(
                    "Unsupported proxy group type: {}",
                    p.proxy_group_type
                ))
            }
        })
    }

    fn rule_to_rule(&self, r: &str) -> Result<ConfigRuleItem> {
        let bad_rule = || anyhow!("Bad rule.");
        let mut ps = r.split(",");
        let mut ps_next = || ps.next().ok_or_else(bad_rule);
        let rule_type = ps_next()?;
        let get_target = |target: &str| -> Result<String> {
            if target == "DIRECT" {
                return Ok(self.direct.clone().unwrap_or("local".to_string()));
            }
            // TODO: noop is not reject, add blackhole net
            if target == "REJECT" {
                return Ok(self.direct.clone().unwrap_or("noop".to_string()));
            }
            let net_name = self.name_map.get(target);
            net_name
                .map(|i| i.to_string())
                .ok_or(anyhow!("Name not found. clash name: {}", target))
        };
        let item = match rule_type {
            "DOMAIN-SUFFIX" | "DOMAIN-KEYWORD" | "DOMAIN" => {
                let domain = ps_next()?;
                let target = get_target(ps_next()?)?;
                let method = match rule_type {
                    "DOMAIN-SUFFIX" => "suffix",
                    "DOMAIN-KEYWORD" => "keyword",
                    "DOMAIN" => "match",
                    _ => return Err(bad_rule()),
                }
                .to_string();
                ConfigRuleItem {
                    rule_type: "domain_suffix".to_string(),
                    target,
                    rest: json!({ "domain": domain, "method": method }),
                }
            }
            "IP-CIDR" | "IP-CIDR6" => {
                let ip_cidr = ps_next()?;
                let target = get_target(ps_next()?)?;
                ConfigRuleItem {
                    rule_type: "ip_cidr".to_string(),
                    target,
                    rest: json!({ "ip_cidr": ip_cidr }),
                }
            }
            "MATCH" => {
                let target = get_target(ps_next()?)?;
                ConfigRuleItem {
                    rule_type: "any".to_string(),
                    target,
                    rest: Value::Null,
                }
            }
            _ => return Err(anyhow!("Rule prefix {} is not supported", rule_type)),
        };
        Ok(item)
    }

    fn proxy_group_name(&self, pg: impl AsRef<str>) -> String {
        format!("proxy_groups.{}", pg.as_ref())
    }

    fn prefix(&self, s: impl AsRef<str>) -> String {
        match &self.prefix {
            Some(prefix) => format!("{}.{}", prefix, s.as_ref()),
            None => s.as_ref().to_string(),
        }
    }

    pub async fn process(&mut self, config: &mut Config, content: String) -> Result<()> {
        let clash_config: ClashConfig = serde_yaml::from_str(&content)?;
        for p in clash_config.proxies {
            let old_name = p.name.clone();
            match self.proxy_to_net(p) {
                Ok((name, p)) => {
                    let name = self.prefix(name);
                    self.name_map.insert(old_name, name.clone());
                    config.net.insert(name, p);
                }
                Err(e) => log::warn!("proxy not translated: {:?}", e),
            };
        }

        for pg in clash_config.proxy_groups {
            let old_name = pg.name.clone();
            match self.proxy_group_to_composite(pg) {
                Ok((name, rule)) => {
                    self.name_map.insert(old_name, name.clone());
                    config.composite.insert(self.prefix(name), rule);
                }
                Err(e) => log::warn!("proxy_group not translated: {:?}", e),
            };
        }

        let mut rule = Vec::<ConfigRuleItem>::new();
        for r in clash_config.rules {
            match self.rule_to_rule(&r) {
                Ok(r) => {
                    rule.push(r);
                }
                Err(e) => log::warn!("rule '{}' not translated: {:?}", r, e),
            }
        }
        config.composite.insert(
            self.prefix("rule"),
            Composite {
                name: None,
                composite_type: "rule".to_string(),
                rest: json!({ "rule": rule }),
            },
        );

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
