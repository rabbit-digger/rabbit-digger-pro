use std::convert::TryFrom;

use super::config::{DomainMatcher, DomainMatcherMethod as Method};
use super::matcher::{MatchContext, Matcher, MaybeAsync};
use anyhow::Result;

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

impl DomainMatcher {
    fn test(&self, domain: &str) -> bool {
        match self.method {
            Method::Keyword => domain.contains(&self.domain),
            Method::Match => domain == self.domain,
            Method::Suffix => domain.ends_with(&self.domain),
        }
    }
}

impl Matcher for DomainMatcher {
    fn match_rule(&self, match_context: &MatchContext) -> MaybeAsync<bool> {
        match match_context.get_domain() {
            Some((domain, _)) => self.test(domain),
            // if it's not a domain, pass it.
            None => false,
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use rd_interface::{Context, IntoAddress};

    use super::*;

    #[tokio::test]
    async fn test_domain_matcher() {
        // test keyword
        let matcher = DomainMatcher {
            domain: "example".to_string(),
            method: Method::Keyword,
        };
        let mut match_context = MatchContext::from_context_address(
            &Context::new(),
            &"example.com:26666".into_address().unwrap(),
        )
        .unwrap();
        assert!(matcher.match_rule(&mut match_context).await);

        let mut match_context = MatchContext::from_context_address(
            &Context::new(),
            &"exampl.com:26666".into_address().unwrap(),
        )
        .unwrap();
        assert!(!matcher.match_rule(&mut match_context).await);

        // test match
        let matcher = DomainMatcher {
            domain: "example.com".to_string(),
            method: Method::Match,
        };
        let mut match_context = MatchContext::from_context_address(
            &Context::new(),
            &"example.com:26666".into_address().unwrap(),
        )
        .unwrap();
        assert!(matcher.match_rule(&mut match_context).await);

        let mut match_context = MatchContext::from_context_address(
            &Context::new(),
            &"sub.example.com:26666".into_address().unwrap(),
        )
        .unwrap();
        assert!(!matcher.match_rule(&mut match_context).await);

        // test suffix
        let matcher = DomainMatcher {
            domain: ".com".to_string(),
            method: Method::Suffix,
        };
        let mut match_context = MatchContext::from_context_address(
            &Context::new(),
            &"example.com:26666".into_address().unwrap(),
        )
        .unwrap();
        assert!(matcher.match_rule(&mut match_context).await);

        let mut match_context = MatchContext::from_context_address(
            &Context::new(),
            &"example.cn:26666".into_address().unwrap(),
        )
        .unwrap();
        assert!(!matcher.match_rule(&mut match_context).await);
    }
}
