mod any;
mod domain;
mod ip_cidr;
mod matcher;

use self::matcher::BoxMatcher;
use self::{any::AnyMatcher, domain::DomainMatcher, ip_cidr::IPMatcher};
use crate::config::{CompositeRule, CompositeRuleItem};
use std::{collections::HashMap, io};

use rd_interface::{
    async_trait, context::common_field::SourceAddress, Address, Arc, Context, INet, Net, Result,
    TcpListener, TcpStream, UdpSocket, NOT_IMPLEMENTED,
};

struct RuleItem {
    _rule_type: String,
    target_name: String,
    target: Net,
    matcher: BoxMatcher,
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
    pub fn new(net: HashMap<String, Net>, config: CompositeRule) -> anyhow::Result<Net> {
        let registry = get_matcher_registry();
        let rule = config
            .rule
            .into_iter()
            .map(
                |CompositeRuleItem {
                     rule_type,
                     target,
                     rest,
                 }| {
                    Ok(RuleItem {
                        matcher: registry.get(&rule_type, rest)?,
                        _rule_type: rule_type,
                        target: net
                            .get(&target)
                            .ok_or(anyhow::anyhow!("target is not found: {}", target))?
                            .to_owned(),
                        target_name: target,
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
            .await
            .map(|s| s.addr.to_string())
            .unwrap_or_default();

        for rule in self.rule.iter() {
            if rule.matcher.match_rule(ctx, &addr).await {
                log::info!(
                    "[{}] {} -> {} matched rule: {}",
                    &rule.target_name,
                    &src,
                    &addr,
                    &rule.matcher
                );
                let r = rule.target.tcp_connect(ctx, addr.clone()).await;
                if let Err(e) = &r {
                    log::error!("{} -> {} Failed to connect: {:?}", &src, &addr, e);
                }
                return r;
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
