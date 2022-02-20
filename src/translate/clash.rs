use std::{collections::BTreeMap, str::FromStr};

use anyhow::{anyhow, Result};
use rabbit_digger::{
    config::{Config, Net},
    rd_std::rule::config::{
        self as rule_config, AnyMatcher, DomainMatcher, DomainMatcherMethod, GeoIpMatcher, IpCidr,
        IpCidrMatcher, Matcher,
    },
};
use rd_interface::config::NetRef;
use serde::Deserialize;
use serde_json::{from_value, json, Value};

#[derive(Debug, Deserialize)]
pub struct Clash {
    rule_name: Option<String>,
    prefix: Option<String>,
    direct: Option<String>,
    reject: Option<String>,

    #[serde(default)]
    disable_proxy_group: bool,

    /// Make all proxies in the group name
    #[serde(default)]
    select: Option<String>,

    // reverse map from clash name to net name
    #[serde(skip)]
    name_map: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ClashConfig {
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
    Net::new(
        "alias",
        json!({
            "net": "noop"
        }),
    )
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
                Net::new(
                    "shadowsocks",
                    json!({
                        "server": format!("{}:{}", params.server, params.port),
                        "cipher": params.cipher,
                        "password": params.password,
                        "udp": params.udp.unwrap_or_default(),
                    }),
                )
            }
            "trojan" => {
                #[derive(Debug, Deserialize)]
                struct Param {
                    server: String,
                    port: u16,
                    password: String,
                    udp: Option<bool>,
                    sni: Option<String>,
                    skip_cert_verify: Option<bool>,
                }
                let params: Param = serde_json::from_value(p.opt)?;

                Net::new(
                    "trojan",
                    json!({
                        "server": format!("{}:{}", params.server, params.port),
                        "password": params.password,
                        "udp": params.udp.unwrap_or_default(),
                        "sni": params.sni.unwrap_or(params.server),
                        "skip_cert_verify": params.skip_cert_verify.unwrap_or_default(),
                    }),
                )
            }
            _ => return Err(anyhow!("Unsupported proxy type: {}", p.proxy_type)),
        };
        Ok(net)
    }

    fn get_target(&self, target: &str) -> Result<String> {
        if target == "DIRECT" {
            return Ok(self.direct.clone().unwrap_or_else(|| "local".to_string()));
        }
        if target == "REJECT" {
            return Ok(self
                .reject
                .clone()
                .unwrap_or_else(|| "blackhole".to_string()));
        }
        let net_name = self.name_map.get(target);
        net_name
            .map(|i| i.to_string())
            .ok_or_else(|| anyhow!("Name not found. clash name: {}", target))
    }

    fn proxy_group_to_net(&self, p: ProxyGroup) -> Result<Net> {
        let net_list = p
            .proxies
            .into_iter()
            .map(|name| self.get_target(&name))
            .collect::<Result<Vec<String>>>()?;
        let proxy_group_type = p.proxy_group_type.as_ref();

        Ok(match proxy_group_type {
            "select" => Net::new(
                "select",
                json!({
                    "selected": net_list.get(0).cloned().unwrap_or_else(|| "noop".to_string()),
                    "list": net_list,
                }),
            ),
            "url-test" | "fallback" => {
                tracing::warn!(
                    "Unsupported proxy group type: {}, will use select as fallback.",
                    proxy_group_type
                );
                Net::new(
                    "select",
                    json!({
                        "selected": net_list.get(0).cloned().unwrap_or_else(|| "noop".to_string()),
                        "list": net_list,
                    }),
                )
            }
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
        let mut ps = r.split(',');
        let mut ps_next = || ps.next().ok_or_else(bad_rule);
        let rule_type = ps_next()?;
        let item = match rule_type {
            "DOMAIN-SUFFIX" | "DOMAIN-KEYWORD" | "DOMAIN" => {
                let domain = ps_next()?.to_string();
                let target = NetRef::new(self.get_target(ps_next()?)?.into());
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
                let target = NetRef::new(self.get_target(ps_next()?)?.into());
                rule_config::RuleItem {
                    target,
                    matcher: Matcher::IpCidr(IpCidrMatcher {
                        ipcidr: IpCidr::from_str(&ip_cidr)?,
                    }),
                }
            }
            "MATCH" => {
                let target = NetRef::new(self.get_target(ps_next()?)?.into());
                rule_config::RuleItem {
                    target,
                    matcher: Matcher::Any(AnyMatcher {}),
                }
            }
            "GEOIP" => {
                let region = ps_next()?.to_string();
                let target = NetRef::new(self.get_target(ps_next()?)?.into());
                rule_config::RuleItem {
                    target,
                    matcher: Matcher::GeoIp(GeoIpMatcher { country: region }),
                }
            }
            _ => return Err(anyhow!("Rule prefix {} is not supported", rule_type)),
        };
        Ok(item)
    }

    fn proxy_group_name(&self, pg: impl AsRef<str>) -> String {
        self.prefix(pg)
    }

    fn prefix(&self, s: impl AsRef<str>) -> String {
        match &self.prefix {
            Some(prefix) => format!("{}.{}", prefix, s.as_ref()),
            None => s.as_ref().to_string(),
        }
    }

    pub async fn process(&mut self, config: &mut Config, content: String) -> Result<()> {
        let clash_config: ClashConfig = serde_yaml::from_str(&content)?;
        let mut added_proxies = Vec::new();

        for p in clash_config.proxies {
            let old_name = p.name.clone();
            let name = self.prefix(&old_name);
            added_proxies.push(name.clone());
            self.name_map.insert(old_name.clone(), name.clone());
            match self.proxy_to_net(p) {
                Ok(p) => {
                    config.net.insert(name, p);
                }
                Err(e) => {
                    tracing::warn!("proxy {} not translated: {:?}", old_name, e);
                    config.net.insert(name, ghost_net());
                }
            };
        }

        if !self.disable_proxy_group {
            for old_name in clash_config.proxy_groups.iter().map(|i| &i.name) {
                let name = self.proxy_group_name(old_name);
                self.name_map.insert(old_name.clone(), name.clone());
            }

            let proxy_groups = clash_config
                .proxy_groups
                .into_iter()
                .map(|i| (i.name.to_string(), i))
                .collect::<Vec<_>>();

            for (old_name, pg) in proxy_groups {
                let name = self.proxy_group_name(&old_name);
                match self.proxy_group_to_net(pg) {
                    Ok(pg) => {
                        config.net.insert(name, pg);
                    }
                    Err(e) => {
                        tracing::warn!("proxy_group {} not translated: {:?}", old_name, e);
                    }
                };
            }
        }

        if let Some(rule_name) = &self.rule_name {
            let mut rule = Vec::new();
            for r in clash_config.rules {
                match self.rule_to_rule(&r) {
                    Ok(r) => {
                        rule.push(r);
                    }
                    Err(e) => tracing::warn!("rule '{}' not translated: {:?}", r, e),
                }
            }
            config
                .net
                .insert(rule_name.clone(), Net::new("rule", json!({ "rule": rule })));
        }

        if let Some(select) = &self.select {
            config.net.insert(
                select.clone(),
                Net::new(
                    "select",
                    json!({
                        "selected": added_proxies.get(0).cloned().unwrap_or_else(|| "noop".to_string()),
                        "list": added_proxies,
                    }),
                ),
            );
        }

        Ok(())
    }
}
