use self::matcher::BoxMatcher;
use self::{any::AnyMatcher, domain::DomainMatcher, ip::IPMatcher};
use crate::{config, registry};
use std::{collections::HashMap, io};

mod any;
mod domain;
mod ip;
mod matcher;

use rd_interface::{
    async_trait, config::Value, context::common_field::SourceAddress, Address, Arc, Context, INet,
    Net, Result, TcpListener, TcpStream, UdpSocket, NOT_IMPLEMENTED,
};

struct RuleItem {
    rule_type: String,
    target_name: String,
    target: Net,
    matcher: BoxMatcher,
    value: Value,
}

fn get_matcher_registry() -> matcher::MatcherRegistry {
    let mut registry = matcher::MatcherRegistry::new();

    DomainMatcher::register(&mut registry);
    AnyMatcher::register(&mut registry);
    IPMatcher::register(&mut registry);

    registry
}

pub struct Rule {
    rule: Vec<RuleItem>,
}

impl Rule {
    pub fn new(net: HashMap<String, Net>, config: config::ConfigRule) -> anyhow::Result<Net> {
        let registry = get_matcher_registry();
        let rule = config
            .into_iter()
            .map(
                |config::Rule {
                     rule_type,
                     target,
                     rest,
                 }| {
                    Ok(RuleItem {
                        matcher: registry.get(&rule_type, rest.clone())?,
                        rule_type,
                        target: net
                            .get(&target)
                            .ok_or(anyhow::anyhow!("target is not found: {}", target))?
                            .to_owned(),
                        target_name: target,
                        value: rest,
                    })
                },
            )
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(Arc::new(Rule { rule }))
    }
}

#[async_trait]
impl INet for Rule {
    async fn tcp_connect(&self, ctx: &mut Context, addr: Address) -> Result<TcpStream> {
        let src = ctx
            .get_common::<SourceAddress>()
            .map(|s| s.addr.to_string())
            .unwrap_or_default();

        for rule in self.rule.iter() {
            if rule.matcher.match_rule(ctx, &addr).await {
                log::info!(
                    "[{}] {} -> {} matched rule {} {}",
                    &rule.target_name,
                    &src,
                    &addr,
                    &rule.rule_type,
                    &rule.value
                );
                return rule.target.tcp_connect(ctx, addr).await;
            }
        }

        log::info!("{} -> {} not matched, reject", src, addr);
        Err(rd_interface::Error::IO(
            io::ErrorKind::ConnectionRefused.into(),
        ))
    }

    async fn tcp_bind(&self, _ctx: &mut Context, _addr: Address) -> Result<TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(&self, _ctx: &mut Context, _addr: Address) -> Result<UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
}
