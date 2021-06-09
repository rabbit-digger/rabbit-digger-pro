use std::net::SocketAddr;

use super::config::GeoIpMatcher;
use super::matcher::{Matcher, MaybeAsync};
use rd_interface::{impl_empty_net_resolve, Address};
use smoltcp::wire::IpAddress;

impl_empty_net_resolve! {GeoIpMatcher}

impl GeoIpMatcher {
    fn test(&self, _address: impl Into<IpAddress>) -> bool {
        false
    }
}

impl Matcher for GeoIpMatcher {
    fn match_rule(&self, _ctx: &rd_interface::Context, addr: &Address) -> MaybeAsync<bool> {
        match addr {
            Address::SocketAddr(addr) => self.test(addr.ip()),
            // if it's a domain, try to parse it to SocketAddr.
            Address::Domain(domain, _) => match str::parse::<SocketAddr>(domain) {
                Ok(addr) => self.test(addr.ip()),
                Err(_) => false,
            },
        }
        .into()
    }
}
