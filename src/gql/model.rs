use crate::{config, controller::Inner};
use async_graphql::{Interface, Object, Result};
use async_std::sync::RwLockReadGuard;

pub struct IdNet<'a>(&'a str, &'a config::Net);
pub struct IdServer<'a>(&'a str, &'a config::Server);

#[Object(name = "Net")]
impl<'a> IdNet<'a> {
    async fn id(&self) -> &str {
        &self.0
    }
    async fn r#type(&self) -> &str {
        &self.1.net_type
    }
    async fn chain(&self) -> Vec<String> {
        self.1.chain.to_vec()
    }
    // TODO: rest
}
#[Object(name = "Server")]
impl<'a> IdServer<'a> {
    async fn id(&self) -> &str {
        &self.0
    }
    async fn r#type(&self) -> &str {
        &self.1.server_type
    }
    async fn listen(&self) -> &str {
        &self.1.listen
    }
    async fn net(&self) -> &str {
        &self.1.net
    }
    // TODO: rest
}

struct CompositeRule<'a>(&'a str, &'a config::CompositeRule);

#[Object]
impl<'a> CompositeRule<'a> {
    async fn id(&self) -> &str {
        &self.0
    }
}

struct CompositeSelect<'a>(&'a str, &'a config::NetList);

#[Object]
impl<'a> CompositeSelect<'a> {
    async fn id(&self) -> &str {
        &self.0
    }
    async fn net_list(&self) -> Vec<&str> {
        self.1.as_ref()
    }
}

#[derive(Interface)]
#[graphql(field(name = "id", type = "&str"))]
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
    async fn net(&'a self) -> Result<Vec<IdNet<'a>>> {
        let config = self.cfg();
        let net_list = config
            .net
            .iter()
            .map(|(k, v)| IdNet(k, v))
            .collect::<Vec<_>>();

        Ok(net_list)
    }
    async fn server(&'a self) -> Result<Vec<IdServer<'a>>> {
        let config = self.cfg();
        let server_list = config
            .server
            .iter()
            .map(|(k, v)| IdServer(k, v))
            .collect::<Vec<_>>();

        Ok(server_list)
    }
    async fn composite(&'a self) -> Result<Vec<Composite<'a>>> {
        let config = self.cfg();
        let server_list = config
            .composite
            .iter()
            .map(|(k, v)| match &v.composite.0 {
                config::Composite::Rule(rule) => Composite::Rule(CompositeRule(k, &rule)),
                config::Composite::Select => Composite::Select(CompositeSelect(k, &v.net_list)),
            })
            .collect::<Vec<_>>();

        Ok(server_list)
    }
}
