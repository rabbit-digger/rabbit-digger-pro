use super::config;
use super::matcher::Matcher;
use super::udp::UdpRuleSocket;
use std::io;

use rd_interface::{
    async_trait, context::common_field::SourceAddress, Address, Arc, Context, INet, IntoDyn, Net,
    Result, TcpListener, TcpStream, UdpSocket, NOT_IMPLEMENTED,
};

pub struct RuleItem {
    pub target_name: String,
    pub target: Net,
    matcher: config::Matcher,
}

#[derive(Clone)]
pub struct Rule {
    rule: Arc<Vec<RuleItem>>,
}

impl Rule {
    fn new(config: config::RuleConfig) -> Result<Rule> {
        let rule = config
            .rule
            .into_iter()
            .map(|config::RuleItem { target, matcher }| {
                Ok(RuleItem {
                    matcher,
                    target: target.net(),
                    target_name: target.name().to_string(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let rule = Arc::new(rule);

        Ok(Rule { rule })
    }
    pub async fn get_rule(&self, ctx: &Context, target: &Address) -> Result<&RuleItem> {
        let src = ctx
            .get_common::<SourceAddress>()
            .map(|s| s.addr.to_string())
            .unwrap_or_default();

        for rule in self.rule.iter() {
            if rule.matcher.match_rule(ctx, &target).await {
                tracing::trace!(
                    "[{}] {} -> {} matched rule: {:?}",
                    &rule.target_name,
                    &src,
                    &target,
                    &rule.matcher
                );
                return Ok(&rule);
            }
        }

        tracing::info!("{} -> {} not matched, reject", src, target);
        Err(rd_interface::Error::IO(
            io::ErrorKind::ConnectionRefused.into(),
        ))
    }
    pub async fn get_rule_append(&self, ctx: &mut Context, target: &Address) -> Result<&RuleItem> {
        let rule = self.get_rule(ctx, target).await?;
        Ok(rule)
    }
}

pub struct RuleNet {
    rule: Rule,
}

impl RuleNet {
    pub fn new(config: config::RuleConfig) -> Result<RuleNet> {
        Ok(RuleNet {
            rule: Rule::new(config)?,
        })
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
            .get_rule_append(ctx, &addr)
            .await?
            .target
            .tcp_connect(ctx, addr.clone())
            .await;

        if let Err(e) = &r {
            tracing::error!("{} -> {} Failed to connect: {:?}", &src, &addr, e);
        }

        r
    }

    async fn tcp_bind(&self, _ctx: &mut Context, _addr: Address) -> Result<TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: Address) -> Result<UdpSocket> {
        Ok(UdpRuleSocket::new(self.rule.clone(), ctx.clone(), addr).into_dyn())
    }
}
