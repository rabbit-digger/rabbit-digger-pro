use super::matcher::{self, BoxMatcher};
use super::udp::UdpRuleSocket;
use super::{any::AnyMatcher, domain::DomainMatcher, ip_cidr::IPMatcher};
use crate::config::{CompositeRule, CompositeRuleItem};
use std::{collections::HashMap, io};

use rd_interface::{
    async_trait, context::common_field::SourceAddress, Address, Arc, Context, INet, IntoDyn, Net,
    Result, TcpListener, TcpStream, UdpSocket, NOT_IMPLEMENTED,
};

pub struct RuleItem {
    pub rule_type: String,
    pub target_name: String,
    pub target: Net,
    matcher: BoxMatcher,
}

fn get_matcher_registry() -> matcher::MatcherRegistry {
    let mut registry = matcher::MatcherRegistry::new();

    DomainMatcher::register(&mut registry);
    AnyMatcher::register(&mut registry);
    IPMatcher::register(&mut registry);

    registry
}

#[derive(Clone)]
pub struct Rule {
    rule: Arc<Vec<RuleItem>>,
}

impl Rule {
    fn new(net: HashMap<String, Net>, config: CompositeRule) -> anyhow::Result<Rule> {
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
                        rule_type,
                        target: net
                            .get(&target)
                            .ok_or(anyhow::anyhow!("target is not found: {}", target))?
                            .to_owned(),
                        target_name: target,
                    })
                },
            )
            .collect::<anyhow::Result<Vec<_>>>()?;
        let rule = Arc::new(rule);

        Ok(Rule { rule })
    }
    pub async fn get_rule(&self, ctx: &mut Context, target: &Address) -> Result<&RuleItem> {
        let src = ctx
            .get_common::<SourceAddress>()
            .map(|s| s.addr.to_string())
            .unwrap_or_default();

        for rule in self.rule.iter() {
            if rule.matcher.match_rule(ctx, &target).await {
                log::info!(
                    "[{}] {} -> {} matched rule: {}",
                    &rule.target_name,
                    &src,
                    &target,
                    &rule.matcher
                );
                ctx.append_composite(&rule.target_name);
                return Ok(&rule);
            }
        }

        log::info!("{} -> {} not matched, reject", src, target);
        Err(rd_interface::Error::IO(
            io::ErrorKind::ConnectionRefused.into(),
        ))
    }
}

pub struct RuleNet {
    rule: Rule,
}

impl RuleNet {
    pub fn new(net: HashMap<String, Net>, config: CompositeRule) -> anyhow::Result<Net> {
        Ok(RuleNet {
            rule: Rule::new(net, config)?,
        }
        .into_dyn())
    }
}

#[async_trait]
impl INet for RuleNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: Address) -> Result<TcpStream> {
        let src = ctx
            .get_common::<SourceAddress>()
            .map(|s| s.addr.to_string())
            .unwrap_or_default();

        let r = self
            .rule
            .get_rule(ctx, &addr)
            .await?
            .target
            .tcp_connect(ctx, addr.clone())
            .await;

        if let Err(e) = &r {
            log::error!("{} -> {} Failed to connect: {:?}", &src, &addr, e);
        }

        r
    }

    async fn tcp_bind(&self, _ctx: &mut Context, _addr: Address) -> Result<TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(&self, ctx: &mut Context, _addr: Address) -> Result<UdpSocket> {
        Ok(UdpRuleSocket::new(self.rule.clone(), ctx.clone()).into_dyn())
    }
}
