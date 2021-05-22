use std::net::SocketAddr;

use super::config::IPMatcher;
use super::matcher::{Matcher, MaybeAsync};
use rd_interface::{registry::ResolveNetRef, Address};
use smoltcp::wire::IpAddress;

impl ResolveNetRef for IPMatcher {}

impl IPMatcher {
    fn test(&self, address: impl Into<IpAddress>) -> bool {
        let address: IpAddress = address.into();
        return self.ipcidr.0.contains_addr(&address);
    }
}

impl Matcher for IPMatcher {
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
