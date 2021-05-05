use std::{fmt, net::SocketAddr, str::FromStr};

use super::matcher::{Matcher, MaybeAsync};
use anyhow::{anyhow, Result};
use rd_interface::Address;
use smoltcp::wire::{IpAddress, IpCidr};

#[derive(Debug)]
pub struct IPMatcher {
    ip_cidr: IpCidr,
}

impl IPMatcher {
    pub fn new(ip_cidr: String) -> Result<IPMatcher> {
        Ok(IPMatcher {
            ip_cidr: IpCidr::from_str(&ip_cidr).map_err(|_| anyhow!("Failed to parse ip_cidr"))?,
        })
    }
    fn test(&self, address: impl Into<IpAddress>) -> bool {
        let address: IpAddress = address.into();
        return self.ip_cidr.contains_addr(&address);
    }
}

impl fmt::Display for IPMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ip_cidr({})", self.ip_cidr)
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
