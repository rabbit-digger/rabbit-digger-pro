use std::{net::IpAddr, sync::Arc, time::Duration};

use dns_parser::{Packet, RData};
use lru_time_cache::LruCache;
use parking_lot::Mutex;

struct Inner {
    records: LruCache<IpAddr, String>,
    cname_map: LruCache<String, String>,
}

#[derive(Clone)]
pub struct ReverseLookup {
    inner: Arc<Mutex<Inner>>,
}

impl ReverseLookup {
    pub fn new() -> ReverseLookup {
        ReverseLookup {
            inner: Arc::new(Mutex::new(Inner {
                records: LruCache::with_expiry_duration_and_capacity(
                    Duration::from_secs(10 * 60),
                    128,
                ),
                cname_map: LruCache::with_expiry_duration_and_capacity(
                    Duration::from_secs(10 * 60),
                    128,
                ),
            })),
        }
    }
    pub fn record_packet(&self, packet: &[u8]) {
        let packet = match Packet::parse(packet) {
            Ok(packet) if packet.questions.len() == 1 => packet,
            _ => return,
        };

        // It seems to be ok to assume that only one question is present
        // https://stackoverflow.com/questions/4082081/requesting-a-and-aaaa-records-in-single-dns-query/4083071#4083071
        let domain = packet.questions.first().unwrap().qname.to_string();

        let Inner { records, cname_map } = &mut *self.inner.lock();
        for ans in packet.answers {
            match ans.data {
                RData::A(addr) => {
                    records.insert(addr.0.into(), domain.to_string());
                }
                RData::AAAA(addr) => {
                    records.insert(addr.0.into(), domain.to_string());
                }
                RData::CNAME(cname) => {
                    cname_map.insert(cname.0.to_string(), domain.to_string());
                }
                _ => {}
            };
        }
    }
    pub fn reverse_lookup(&self, addr: IpAddr) -> Option<String> {
        let Inner { records, cname_map } = &mut *self.inner.lock();
        match records.get(&addr) {
            Some(mut d) => {
                let mut limit = 16;
                while let Some(r) = cname_map.peek(d) {
                    if r == d || limit == 0 {
                        break;
                    }
                    d = r;
                    limit -= 1;
                }
                Some(d.to_string())
            }
            None => None,
        }
    }
}
