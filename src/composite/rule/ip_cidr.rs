use std::{fmt, net::SocketAddr, str::FromStr};

use super::matcher::{Matcher, MaybeAsync};
use anyhow::Result;
use rd_interface::Address;
use serde::de::{self, Deserialize, Deserializer};
use serde_derive::Deserialize;
use serde_json::from_str;
use smoltcp::wire::{IpAddress, IpCidr};

#[derive(Debug)]
struct WrapIpCidr(IpCidr);

impl FromStr for WrapIpCidr {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        IpCidr::from_str(s)
            .map(WrapIpCidr)
            .map_err(|_| "Failed to parse ip_cidr".to_string())
    }
}

impl<'de> Deserialize<'de> for WrapIpCidr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

#[derive(Debug, Deserialize)]
pub struct IPMatcher {
    ip_cidr: WrapIpCidr,
}

impl IPMatcher {
    pub fn new(ip_cidr: String) -> Result<IPMatcher> {
        Ok(IPMatcher {
            ip_cidr: from_str(&ip_cidr)?,
        })
    }
    fn test(&self, address: impl Into<IpAddress>) -> bool {
        let address: IpAddress = address.into();
        return self.ip_cidr.0.contains_addr(&address);
    }
}

impl fmt::Display for IPMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ip_cidr({})", self.ip_cidr.0)
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
