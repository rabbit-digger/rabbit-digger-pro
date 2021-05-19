use std::{collections::HashMap, str::FromStr};

use anyhow::{anyhow, Result};
use rabbit_digger::{
    config::{Config, Net, Server},
    rd_std::rule::config::{
        self as rule_config, AnyMatcher, DomainMatcher, DomainMatcherMethod, IPMatcher, IpCidr,
        Matcher,
    },
    util::topological_sort,
};
use serde_derive::Deserialize;
use serde_json::{from_value, json, Value};

#[derive(Debug, Deserialize)]
pub struct Clash {
    prefix: Option<String>,
    target: Option<String>,
    direct: Option<String>,
    #[serde(default)]
    disable_socks5: bool,
    #[serde(default)]
    disable_proxy_group: bool,
    #[serde(default)]
    disable_rule: bool,
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
    opt: Value,
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

fn ghost_net() -> Net {
    Net {
        net_type: "alias".to_string(),
        opt: json!({
            "net": "noop"
        }),
    }
}

impl Clash {
    fn proxy_to_net(&self, p: Proxy) -> Result<Net> {
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
                let params: Param = serde_json::from_value(p.opt)?;
                Net {
                    net_type: "shadowsocks".to_string(),
                    opt: json!({
                        "server": params.server,
                        "port": params.port,
                        "cipher": params.cipher,
                        "password": params.password,
                        "udp": params.udp.unwrap_or_default(),
                    }),
                }
            }
            _ => return Err(anyhow!("Unsupported proxy type: {}", p.proxy_type)),
        };
        Ok(net)
    }

    fn get_target(&self, target: &str) -> Result<String> {
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
    }

    fn proxy_group_to_net(&self, p: ProxyGroup) -> Result<Net> {
        let net_list = p
            .proxies
            .into_iter()
            .map(|name| self.get_target(&name))
            .collect::<Result<Vec<String>>>()?;

        Ok(match p.proxy_group_type.as_ref() {
            "select" => Net {
                net_type: "select".to_string(),
                opt: json!({
                    "net_list": net_list,
                }),
            },
            _ => {
                return Err(anyhow!(
                    "Unsupported proxy group type: {}",
                    p.proxy_group_type
                ))
            }
        })
    }

    fn rule_to_rule(&self, r: &str) -> Result<rule_config::RuleItem> {
        let bad_rule = || anyhow!("Bad rule.");
        let mut ps = r.split(",");
        let mut ps_next = || ps.next().ok_or_else(bad_rule);
        let rule_type = ps_next()?;
        let item = match rule_type {
            "DOMAIN-SUFFIX" | "DOMAIN-KEYWORD" | "DOMAIN" => {
                let domain = ps_next()?.to_string();
                let target = self.get_target(ps_next()?)?.into();
                let method = match rule_type {
                    "DOMAIN-SUFFIX" => DomainMatcherMethod::Suffix,
                    "DOMAIN-KEYWORD" => DomainMatcherMethod::Keyword,
                    "DOMAIN" => DomainMatcherMethod::Match,
                    _ => return Err(bad_rule()),
                };
                rule_config::RuleItem {
                    target,
                    matcher: Matcher::Domain(DomainMatcher { method, domain }),
                }
            }
            "IP-CIDR" | "IP-CIDR6" => {
                let ip_cidr = ps_next()?.to_string();
                let target = self.get_target(ps_next()?)?.into();
                rule_config::RuleItem {
                    target,
                    matcher: Matcher::IpCidr(IPMatcher {
                        ip_cidr: IpCidr::from_str(&ip_cidr)?,
                    }),
                }
            }
            "MATCH" => {
                let target = self.get_target(ps_next()?)?.into();
                rule_config::RuleItem {
                    target,
                    matcher: Matcher::Any(AnyMatcher {}),
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
            let name = self.prefix(&old_name);
            self.name_map.insert(old_name.clone(), name.clone());
            match self.proxy_to_net(p) {
                Ok(p) => {
                    config.net.insert(name, p);
                }
                Err(e) => {
                    log::warn!("proxy {} not translated: {:?}", old_name, e);
                    config.net.insert(name, ghost_net());
                }
            };
        }

        if !self.disable_proxy_group {
            let proxy_groups = topological_sort(
                clash_config
                    .proxy_groups
                    .into_iter()
                    .map(|i| (i.name.clone(), i))
                    .collect(),
                |i: &ProxyGroup| Ok(i.proxies.clone()) as Result<_>,
            )?
            .ok_or(anyhow!("There is cyclic dependencies in proxy_groups"))?;

            for (old_name, pg) in proxy_groups {
                let name = self.proxy_group_name(&old_name);
                match self.proxy_group_to_net(pg) {
                    Ok(pg) => {
                        self.name_map.insert(old_name, name.clone());
                        config.net.insert(name, pg);
                    }
                    Err(e) => {
                        log::warn!("proxy_group {} not translated: {:?}", old_name, e);
                    }
                };
            }
        }

        if !self.disable_rule {
            let mut rule = Vec::new();
            for r in clash_config.rules {
                match self.rule_to_rule(&r) {
                    Ok(r) => {
                        rule.push(r);
                    }
                    Err(e) => log::warn!("rule '{}' not translated: {:?}", r, e),
                }
            }
            config.net.insert(
                self.prefix("clash_rule"),
                Net {
                    net_type: "rule".to_string(),
                    opt: json!({ "rule": rule }),
                },
            );
        }

        if !self.disable_socks5 {
            config.server.insert(
                self.prefix("socks_port"),
                Server {
                    server_type: "socks5".to_string(),
                    listen: "local".to_string(),
                    net: self.target.clone().unwrap_or(self.prefix("clash_rule")),
                    opt: json!({ "bind": format!("0.0.0.0:{}", clash_config.socks_port) }),
                },
            );
        }

        Ok(())
    }
}
