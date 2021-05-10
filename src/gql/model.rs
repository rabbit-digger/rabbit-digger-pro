use crate::{config, controller::Inner};
use async_graphql::{types::OutputJson, Interface, Object, Result, SimpleObject};
use async_std::sync::RwLockReadGuard;
use serde_json::Value;

#[derive(SimpleObject)]
pub struct Net<'a> {
    id: &'a str,
    r#type: &'a str,
    chain: &'a Vec<String>,
    opt: OutputJson<&'a Value>,
}

#[derive(SimpleObject)]
pub struct Server<'a> {
    id: &'a str,
    r#type: &'a str,
    listen: &'a str,
    net: &'a str,
    opt: OutputJson<&'a Value>,
}

#[derive(SimpleObject)]
struct CompositeRule<'a> {
    id: &'a str,
    name: Option<&'a str>,
    composite_type: &'a str,
    rule: Vec<Matcher<'a>>,
}

#[derive(SimpleObject)]
struct CompositeSelect<'a> {
    id: &'a str,
    name: Option<&'a str>,
    composite_type: &'a str,
    net_list: &'a Vec<String>,
}

#[derive(Interface)]
#[graphql(field(name = "id", type = "&&str"))]
#[graphql(field(name = "name", type = "&Option<&str>"))]
#[graphql(field(name = "composite_type", type = "&&str"))]
enum Composite<'a> {
    Rule(CompositeRule<'a>),
    Select(CompositeSelect<'a>),
}

#[derive(SimpleObject)]
struct MatcherDomain<'a> {
    target: &'a str,
    matcher_type: &'a str,

    method: &'a str,
    domain: &'a str,
}

#[derive(SimpleObject)]
struct MatcherIpCidr<'a> {
    target: &'a str,
    matcher_type: &'a str,

    ip_cidr: &'a str,
}

#[derive(SimpleObject)]
struct MatcherAny<'a> {
    target: &'a str,
    matcher_type: &'a str,
}

#[derive(Interface)]
#[graphql(field(name = "target", type = "&&str"))]
#[graphql(field(name = "matcher_type", type = "&&str"))]
enum Matcher<'a> {
    Domain(MatcherDomain<'a>),
    IpCidr(MatcherIpCidr<'a>),
    Any(MatcherAny<'a>),
}

pub(crate) struct Config<'a>(pub RwLockReadGuard<'a, Inner>);

impl<'a> Config<'a> {
    fn cfg(&self) -> &config::Config {
        &self.0.config().unwrap()
    }
}

#[Object]
impl<'a> Config<'a> {
    async fn net(&'a self) -> Result<Vec<Net<'a>>> {
        let config = self.cfg();
        let net_list = config
            .net
            .iter()
            .map(|(id, v)| Net {
                id,
                r#type: &v.net_type,
                chain: &v.chain,
                opt: OutputJson(&v.opt),
            })
            .collect::<Vec<_>>();

        Ok(net_list)
    }
    async fn server(&'a self) -> Result<Vec<Server<'a>>> {
        let config = self.cfg();
        let server_list = config
            .server
            .iter()
            .map(|(id, v)| Server {
                id,
                r#type: &v.server_type,
                listen: &v.listen,
                net: &v.net,
                opt: OutputJson(&v.opt),
            })
            .collect::<Vec<_>>();

        Ok(server_list)
    }
    async fn composite(&'a self) -> Result<Vec<Composite<'a>>> {
        let config = self.cfg();
        let server_list = config.composite.iter().map(Into::into).collect::<Vec<_>>();

        Ok(server_list)
    }
}

impl<'a> From<(&'a String, &'a config::CompositeName)> for Composite<'a> {
    fn from((k, v): (&'a String, &'a config::CompositeName)) -> Self {
        let k: &str = k;
        match &v.composite.0 {
            config::Composite::Rule(rule) => Composite::Rule(CompositeRule {
                id: k,
                name: v.name.as_ref().map(AsRef::as_ref),
                composite_type: "rule",
                rule: rule.rule.iter().map(Into::into).collect(),
            }),
            config::Composite::Select => Composite::Select(CompositeSelect {
                id: k,
                name: v.name.as_ref().map(AsRef::as_ref),
                composite_type: "select",
                net_list: &v.net_list,
            }),
        }
    }
}

impl<'a> From<&'a config::CompositeRuleItem> for Matcher<'a> {
    fn from(rule: &'a config::CompositeRuleItem) -> Self {
        let target = rule.target.as_ref();

        match &rule.matcher {
            config::Matcher::Domain { method, domain } => Matcher::Domain(MatcherDomain {
                target,
                matcher_type: "domain",

                method,
                domain,
            }),
            config::Matcher::IpCidr { ip_cidr } => Matcher::IpCidr(MatcherIpCidr {
                target,
                matcher_type: "ip_cidr",

                ip_cidr,
            }),
            config::Matcher::Any => Matcher::Any(MatcherAny {
                target,
                matcher_type: "any",
            }),
        }
    }
}
