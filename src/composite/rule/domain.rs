use std::{
    convert::{TryFrom, TryInto},
    fmt,
};

use super::matcher::{Matcher, MaybeAsync};
use anyhow::Result;
use rd_interface::Address;

#[derive(Debug)]
pub enum Method {
    Keyword,
    Suffix,
    Match,
}

impl TryFrom<String> for Method {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(match value.as_ref() {
            "keyword" => Method::Keyword,
            "suffix" => Method::Suffix,
            "match" => Method::Match,
            _ => return Err(anyhow::anyhow!("Unsupported method: {}", value)),
        })
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Method::Keyword => "keyword",
            Method::Suffix => "suffix",
            Method::Match => "prefix",
        })
    }
}

#[derive(Debug)]
pub struct DomainMatcher {
    method: Method,
    domain: String,
}

impl fmt::Display for DomainMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "domain({}, {})", self.method, self.domain)
    }
}

impl DomainMatcher {
    pub fn new(method: String, domain: String) -> Result<DomainMatcher> {
        Ok(DomainMatcher {
            method: method.try_into()?,
            domain,
        })
    }
    fn test(&self, domain: &str) -> bool {
        match self.method {
            Method::Keyword => domain.contains(&self.domain),
            Method::Match => domain == &self.domain,
            Method::Suffix => domain.ends_with(&self.domain),
        }
    }
}

impl Matcher for DomainMatcher {
    fn match_rule(&self, _ctx: &rd_interface::Context, addr: &Address) -> MaybeAsync<bool> {
        match addr {
            Address::Domain(domain, _) => self.test(domain),
            // if it's not a domain, pass it.
            _ => false,
        }
        .into()
    }
}
