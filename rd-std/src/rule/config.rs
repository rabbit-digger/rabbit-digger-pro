use std::{fmt, str::FromStr};

use super::matcher;
use rd_interface::{
    registry::{NetRef, ResolveNetRef},
    schemars::{
        self,
        schema::{InstanceType, SchemaObject},
        JsonSchema,
    },
    Config,
};
use serde_derive::{Deserialize, Serialize};
use serde_with::rust::display_fromstr;
use smoltcp::wire;

#[derive(Debug, Serialize, Deserialize, Clone, Config, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DomainMatcherMethod {
    Keyword,
    Suffix,
    Match,
}

#[derive(Debug, Serialize, Deserialize, Clone, Config, JsonSchema)]
pub struct DomainMatcher {
    pub method: DomainMatcherMethod,
    pub domain: String,
}

#[derive(Debug, Clone)]
pub struct IpCidr(pub wire::IpCidr);

impl fmt::Display for IpCidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for IpCidr {
    type Err = rd_interface::Error;

    /// Parse a string representation of an IP CIDR.
    fn from_str(s: &str) -> rd_interface::Result<IpCidr> {
        match wire::Ipv4Cidr::from_str(s) {
            Ok(cidr) => return Ok(IpCidr(wire::IpCidr::Ipv4(cidr))),
            Err(_) => (),
        }

        match wire::Ipv6Cidr::from_str(s) {
            Ok(cidr) => return Ok(IpCidr(wire::IpCidr::Ipv6(cidr))),
            Err(_) => (),
        }

        Err(rd_interface::Error::Other(
            format!("Failed to parse ip_cidr: {}", s).into(),
        ))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct IPMatcher {
    #[serde(
        serialize_with = "display_fromstr::serialize",
        deserialize_with = "display_fromstr::deserialize"
    )]
    pub ipcidr: IpCidr,
}

impl JsonSchema for IpCidr {
    fn schema_name() -> String {
        "IpCidr".to_string()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            format: None,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Config, JsonSchema)]
pub struct AnyMatcher {}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Matcher {
    Domain(DomainMatcher),
    IpCidr(IPMatcher),
    Any(AnyMatcher),
}

#[derive(Debug, Serialize, Deserialize, Clone, Config, JsonSchema)]
pub struct RuleItem {
    pub target: NetRef,
    #[serde(flatten)]
    pub matcher: Matcher,
}

#[derive(Debug, Serialize, Deserialize, Clone, Config, JsonSchema)]
pub struct RuleConfig {
    pub rule: Vec<RuleItem>,
}

impl ResolveNetRef for Matcher {}

impl matcher::Matcher for Matcher {
    fn match_rule(
        &self,
        ctx: &rd_interface::Context,
        addr: &rd_interface::Address,
    ) -> matcher::MaybeAsync<bool> {
        match self {
            Matcher::Domain(i) => i.match_rule(ctx, addr),
            Matcher::IpCidr(i) => i.match_rule(ctx, addr),
            Matcher::Any(i) => i.match_rule(ctx, addr),
        }
    }
}
