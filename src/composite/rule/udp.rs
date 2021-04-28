use std::{net::SocketAddr, time::Duration};

use super::rule::Rule;
use async_std::sync::RwLock;
use lru_time_cache::LruCache;
use rd_interface::{async_trait, Address, Context, IUdpSocket, Result, UdpSocket, NOT_IMPLEMENTED};

pub struct UdpRuleSocket {
    rule: Rule,
    context: Context,
    nat: RwLock<LruCache<Address, UdpSocket>>,
}

impl UdpRuleSocket {
    pub fn new(rule: Rule, context: Context) -> UdpRuleSocket {
        UdpRuleSocket {
            rule,
            context,
            nat: RwLock::new(LruCache::with_expiry_duration_and_capacity(
                Duration::from_secs(10 * 60),
                100,
            )),
        }
    }
}

#[async_trait]
impl IUdpSocket for UdpRuleSocket {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        Err(NOT_IMPLEMENTED)
    }

    async fn send_to(&self, buf: &[u8], saddr: SocketAddr) -> Result<usize> {
        let mut ctx = self.context.clone();
        let addr: Address = saddr.into();
        let nat_has = self.nat.read().await.contains_key(&addr);

        let udp = if nat_has {
            self.nat.write().await.get(&addr).cloned()
        } else {
            None
        };

        let udp = match udp {
            Some(u) => u,
            None => {
                let r = self
                    .rule
                    .get_rule(&mut ctx, &addr)
                    .await?
                    .target
                    .udp_bind(&mut ctx, addr.clone())
                    .await?;
                self.nat.write().await.insert(addr.clone(), r.clone());
                r
            }
        };

        udp.send_to(buf, saddr).await
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}
