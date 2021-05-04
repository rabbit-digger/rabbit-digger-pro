use crate::{config, controller::Inner};
use async_graphql::{Interface, Object, Result, SimpleObject};
use async_std::sync::RwLockReadGuard;

#[derive(SimpleObject)]
pub struct Net<'a> {
    id: &'a str,
    r#type: &'a str,
    chain: Vec<&'a str>,
    // TODO: rest
}

#[derive(SimpleObject)]
pub struct Server<'a> {
    id: &'a str,
    r#type: &'a str,
    listen: &'a str,
    net: &'a str,
    // TODO: rest
}

#[derive(SimpleObject)]
struct CompositeRule<'a> {
    id: &'a str,
    name: Option<&'a str>,
}

#[derive(SimpleObject)]
struct CompositeSelect<'a> {
    id: &'a str,
    name: Option<&'a str>,
    net_list: Vec<&'a str>,
}

#[derive(Interface)]
#[graphql(field(name = "id", type = "&&str"))]
#[graphql(field(name = "name", type = "&Option<&str>"))]
enum Composite<'a> {
    Rule(CompositeRule<'a>),
    Select(CompositeSelect<'a>),
}

pub(crate) struct Config<'a>(pub RwLockReadGuard<'a, Inner>);

impl<'a> Config<'a> {
    fn cfg(&'a self) -> &config::Config {
        &self.0.config().as_ref().unwrap()
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
                chain: v.chain.as_ref(),
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
            config::Composite::Rule(_rule) => Composite::Rule(CompositeRule {
                id: k,
                name: v.name.as_ref().map(AsRef::as_ref),
            }),
            config::Composite::Select => Composite::Select(CompositeSelect {
                id: k,
                name: v.name.as_ref().map(AsRef::as_ref),
                net_list: v.net_list.as_ref(),
            }),
        }
    }
}
