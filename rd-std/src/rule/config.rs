use std::{fmt, str::FromStr};

use super::matcher;
use rd_interface::{
    impl_empty_config,
    prelude::*,
    registry::NetRef,
    schemars::{
        schema::{InstanceType, SchemaObject},
        JsonSchema,
    },
};
use serde_with::rust::display_fromstr;
use smoltcp::wire;

#[rd_config]
#[derive(Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum DomainMatcherMethod {
    Keyword,
    Suffix,
    Match,
}

#[rd_config]
#[derive(Debug, Clone)]
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
        if let Ok(cidr) = wire::Ipv4Cidr::from_str(s) {
            return Ok(IpCidr(wire::IpCidr::Ipv4(cidr)));
        }

        if let Ok(cidr) = wire::Ipv6Cidr::from_str(s) {
            return Ok(IpCidr(wire::IpCidr::Ipv6(cidr)));
        }

        Err(rd_interface::Error::Other(
            format!("Failed to parse ip_cidr: {}", s).into(),
        ))
    }
}

impl_empty_config! { IpCidr }

#[rd_config]
#[derive(Debug, Clone)]
pub struct IpCidrMatcher {
    #[serde(
        serialize_with = "display_fromstr::serialize",
        deserialize_with = "display_fromstr::deserialize"
    )]
    pub ipcidr: IpCidr,
}

#[rd_config]
#[derive(Debug, Clone)]
pub struct GeoIpMatcher {
    pub country: String,
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

#[rd_config]
#[derive(Debug, Clone)]
pub struct AnyMatcher {}

#[rd_config]
#[derive(Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Matcher {
    Domain(DomainMatcher),
    IpCidr(IpCidrMatcher),
    GeoIp(GeoIpMatcher),
    Any(AnyMatcher),
}

#[rd_config]
#[derive(Debug, Clone)]
pub struct RuleItem {
    pub target: NetRef,
    #[serde(flatten)]
    pub matcher: Matcher,
}

#[rd_config]
#[derive(Debug, Clone)]
pub struct RuleNetConfig {
    pub rule: Vec<RuleItem>,
}

impl matcher::Matcher for Matcher {
    fn match_rule(
        &self,
        ctx: &rd_interface::Context,
        addr: &rd_interface::Address,
    ) -> matcher::MaybeAsync<bool> {
        match self {
            Matcher::Domain(i) => i.match_rule(ctx, addr),
            Matcher::IpCidr(i) => i.match_rule(ctx, addr),
            Matcher::GeoIp(i) => i.match_rule(ctx, addr),
            Matcher::Any(i) => i.match_rule(ctx, addr),
        }
    }
}
