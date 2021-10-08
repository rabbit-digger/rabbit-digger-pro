use crate::rule::matcher::MatchContext;

use super::config;
use super::matcher::Matcher;
use super::udp::UdpRuleSocket;

use lru_time_cache::LruCache;
use parking_lot::Mutex;
use rd_interface::{
    async_trait, Address, Arc, Context, INet, IntoDyn, Net, Result, TcpStream, UdpSocket,
};
use tracing::instrument;

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
        let mut rule = config
            .rule
            .into_iter()
            .map(|config::RuleItem { target, matcher }| {
                Ok(RuleItem {
                    matcher,
                    target: (*target).clone(),
                    target_name: target.represent().to_string(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        rule.shrink_to_fit();

        let rule = Arc::new(rule);
        let cache = Arc::new(Mutex::new(LruCache::with_capacity(config.lru_cache_size)));

        Ok(Rule { rule, cache })
    }
    #[instrument(skip(self), err)]
    pub async fn get_rule(&self, ctx: &Context, target: &Address) -> Result<&RuleItem> {
        let match_context = MatchContext::from_context_address(ctx, target)?;

        // hit cache
        if let Some(i) = self.cache.lock().get(&match_context).copied() {
            let rule = &self.rule[i];
            tracing::trace!(matcher = ?rule.matcher, hit_cache = true, "matched rule");
            return Ok(rule);
        }

        for (i, rule) in self.rule.iter().enumerate() {
            if rule.matcher.match_rule(&match_context).await {
                self.cache.lock().insert(match_context, i);
                tracing::trace!(matcher = ?rule.matcher, hit_cache = false, "matched rule");
                return Ok(&rule);
            }
        }

        tracing::trace!("Not matched");
        Err(rd_interface::Error::NotMatched)
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
        self.rule
            .get_rule(ctx, &addr)
            .await?
            .target
            .tcp_connect(ctx, addr)
            .await
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        Ok(UdpRuleSocket::new(self.rule.clone(), ctx.clone(), addr.clone()).into_dyn())
    }
}
