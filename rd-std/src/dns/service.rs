use std::{net::IpAddr, sync::Arc, time::Duration};

use dns_parser::{Packet, RData};
use lru_time_cache::LruCache;
use parking_lot::Mutex;

#[derive(Clone)]
pub struct ReverseLookup {
    records: Arc<Mutex<LruCache<IpAddr, String>>>,
}

impl ReverseLookup {
    pub fn new() -> ReverseLookup {
        ReverseLookup {
            records: Arc::new(Mutex::new(LruCache::with_expiry_duration_and_capacity(
                Duration::from_secs(10 * 60),
                1024,
            ))),
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

        let mut records = self.records.lock();
        for ans in packet.answers {
            let addr: IpAddr = match ans.data {
                RData::A(addr) => addr.0.into(),
                RData::AAAA(addr) => addr.0.into(),
                _ => continue,
            };

            records.insert(addr, domain.to_string());
        }
    }
    #[allow(dead_code)]
    pub fn record_resolve(&self, domain: &str, addr: Vec<IpAddr>) {
        let mut records = self.records.lock();
        for a in addr {
            records.insert(a, domain.to_string());
        }
    }
    pub fn reverse_lookup(&self, addr: IpAddr) -> Option<String> {
        let mut records = self.records.lock();
        records.get(&addr).cloned()
    }
}
