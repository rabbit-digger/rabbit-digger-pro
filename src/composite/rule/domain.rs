use std::fmt;

use super::matcher::{Matcher, MatcherRegistry, MaybeAsync};
use rd_interface::{config::from_value, Address};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Keyword,
    Suffix,
    Match,
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
        registry.register("domain_keyword", |value| {
            Ok(Box::new({
                let mut this = from_value::<DomainMatcher>(value)?;
                this.method = Method::Keyword;
                this
            }))
        });
        registry.register("domain_match", |value| {
            Ok(Box::new({
                let mut this = from_value::<DomainMatcher>(value)?;
                this.method = Method::Match;
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
