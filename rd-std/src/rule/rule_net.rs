use crate::rule::matcher::MatchContext;

use super::config;
use super::matcher::Matcher;
use super::udp::UdpRuleSocket;
use std::io;
use std::time::Instant;

use lru_time_cache::LruCache;
use parking_lot::Mutex;
use rd_interface::{
    async_trait, Address, Arc, Context, INet, IntoDyn, Net, Result, TcpStream, UdpSocket,
};

pub struct RuleItem {
    pub target_name: String,
    pub target: Net,
    matcher: config::Matcher,
}

#[derive(Clone)]
pub struct Rule {
    rule: Arc<Vec<RuleItem>>,
    cache: Arc<Mutex<LruCache<MatchContext, usize>>>,
}

impl Rule {
    fn new(config: config::RuleNetConfig) -> Result<Rule> {
        if config
            .rule
            .iter()
            .find(|i| matches!(i.matcher, config::Matcher::GeoIp(_)))
            .is_some()
        {
            // if used geoip, init reader first.
            super::geoip::get_reader();
        }
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
        let cache = Arc::new(Mutex::new(LruCache::with_capacity(config.lru_cache_size)));

        Ok(Rule { rule, cache })
    }
    pub async fn get_rule(&self, ctx: &Context, target: &Address) -> Result<&RuleItem> {
        let start = Instant::now();
        let src = ctx
            .get_source_addr()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let match_context = MatchContext::from_context_address(ctx, target);

        // hit cache
        if let Some(i) = self.cache.lock().get(&match_context).copied() {
            let rule = &self.rule[i];
            tracing::trace!(
                "[{}] {} -> {} matched rule: {:?} ({:?})",
                &rule.target_name,
                &src,
                &target,
                &rule.matcher,
                start.elapsed(),
            );
            return Ok(rule);
        }

        for (i, rule) in self.rule.iter().enumerate() {
            if rule.matcher.match_rule(&match_context).await {
                self.cache.lock().insert(match_context, i);
                tracing::trace!(
                    "[{}] {} -> {} matched rule: {:?} ({:?})",
                    &rule.target_name,
                    &src,
                    &target,
                    &rule.matcher,
                    start.elapsed(),
                );
                return Ok(&rule);
            }
        }

        tracing::info!("{} -> {} not matched, reject", src, target);
        Err(rd_interface::Error::IO(
            io::ErrorKind::ConnectionRefused.into(),
        ))
    }
}

pub struct RuleNet {
    rule: Rule,
}

impl RuleNet {
    pub fn new(config: config::RuleNetConfig) -> Result<RuleNet> {
        Ok(RuleNet {
            rule: Rule::new(config)?,
        })
    }
}

#[async_trait]
impl INet for RuleNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: &Address) -> Result<TcpStream> {
        let src = ctx
            .get_source_addr()
            .map(|s| s.to_string())
            .unwrap_or_default();

        let r = self
            .rule
            .get_rule(ctx, &addr)
            .await?
            .target
            .tcp_connect(ctx, addr)
            .await;

        if let Err(e) = &r {
            tracing::error!("{} -> {} Failed to connect: {:?}", &src, addr, e);
        }

        r
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        Ok(UdpRuleSocket::new(self.rule.clone(), ctx.clone(), addr.clone()).into_dyn())
    }
}
