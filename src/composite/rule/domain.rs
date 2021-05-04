use std::fmt;

use super::matcher::{Matcher, MaybeAsync};
use anyhow::Result;
use rd_interface::{config::from_value, Address};
use serde_derive::{Deserialize, Serialize};
use serde_json::from_str;

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
    pub fn new(method: String, domain: String) -> Result<DomainMatcher> {
        Ok(DomainMatcher {
            method: from_str(&method)?,
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
