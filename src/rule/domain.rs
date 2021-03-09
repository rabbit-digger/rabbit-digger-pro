use std::fmt;

use super::matcher::{Matcher, MatcherRegistry, MaybeAsync};
use rd_interface::{config::from_value, Address};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum Method {
    Keyword,
    Suffix,
    Prefix,
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Method::Keyword => "keyword",
            Method::Suffix => "suffix",
            Method::Prefix => "prefix",
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DomainMatcher {
    #[serde(default = "default_method")]
    method: Method,
    domain: String,
}

impl fmt::Display for DomainMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "domain({}, {})", self.method, self.domain)
    }
}

fn default_method() -> Method {
    Method::Suffix
}

impl DomainMatcher {
    pub fn register(registry: &mut MatcherRegistry) {
        registry.register("domain", |value| {
            Ok(Box::new(from_value::<DomainMatcher>(value)?))
        });
        registry.register("domain_prefix", |value| {
            Ok(Box::new({
                let mut this = from_value::<DomainMatcher>(value)?;
                this.method = Method::Prefix;
                this
            }))
        });
        registry.register("domain_suffix", |value| {
            Ok(Box::new({
                let mut this = from_value::<DomainMatcher>(value)?;
                this.method = Method::Suffix;
                this
            }))
        });
    }
    fn test(&self, domain: &str) -> bool {
        match self.method {
            Method::Keyword => domain.contains(&self.domain),
            Method::Prefix => domain.starts_with(&self.domain),
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
