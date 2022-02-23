use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

pub use crate::config::NetRef;
use crate::{
    config::{resolve_net, Config, Visitor, VisitorContext},
    IntoDyn, Net, Result, Server,
};
pub use schemars::JsonSchema;
use schemars::{schema::RootSchema, schema_for};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub type NetGetter<'a> = &'a dyn Fn(&str) -> Option<Net>;

pub struct Registry {
    pub net: BTreeMap<String, NetResolver>,
    pub server: BTreeMap<String, ServerResolver>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("net", &self.net.keys())
            .field("server", &self.server.keys())
            .finish()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Registry {
        Registry {
            net: BTreeMap::new(),
            server: BTreeMap::new(),
        }
    }
    pub fn add_net<N: Builder<Net>>(&mut self) {
        self.net.insert(N::NAME.into(), NetResolver::new::<N>());
    }
    pub fn add_server<S: Builder<Server>>(&mut self) {
        self.server
            .insert(S::NAME.into(), ServerResolver::new::<S>());
    }
}

pub trait Builder<ItemType> {
    const NAME: &'static str;
    type Config: DeserializeOwned + JsonSchema + Config + 'static;
    type Item: IntoDyn<ItemType> + Sized + 'static;

    fn build(config: Self::Config) -> Result<Self::Item>;
}

pub struct Resolver<ItemType> {
    parse_config: fn(cfg: Value) -> Result<Box<dyn Config>>,
    build: fn(getter: NetGetter, cfg: Value) -> Result<ItemType>,
    schema: RootSchema,
}
pub type NetResolver = Resolver<Net>;
pub type ServerResolver = Resolver<Server>;

impl<ItemType> Resolver<ItemType> {
    fn new<N: Builder<ItemType>>() -> Self {
        let schema = schema_for!(N::Config);
        Self {
            parse_config: |cfg| {
                let cfg =
                    serde_json::from_value::<N::Config>(cfg).map_err(Into::<crate::Error>::into)?;
                Ok(Box::new(cfg))
            },
            build: |getter, cfg| {
                serde_json::from_value(cfg)
                    .map_err(Into::<crate::Error>::into)
                    .and_then(|mut cfg: N::Config| {
                        resolve_net(&mut cfg, getter)?;
                        Ok(cfg)
                    })
                    .and_then(N::build)
                    .map(|n| n.into_dyn())
            },
            schema,
        }
    }
    pub fn collect_net_ref(&self, prefix: &str, cfg: Value) -> Result<HashMap<Vec<String>, Value>> {
        let mut to_add = HashMap::new();

        let mut cfg = (self.parse_config)(cfg)?;
        struct ResolveNetRefVisitor<'a>(&'a str, &'a mut HashMap<Vec<String>, Value>);

        impl<'a> Visitor for ResolveNetRefVisitor<'a> {
            fn visit_net_ref(
                &mut self,
                ctx: &mut VisitorContext,
                net_ref: &mut NetRef,
            ) -> Result<()> {
                match net_ref.represent() {
                    Value::String(_) => {}
                    opt => {
                        let mut key = vec![self.0.to_string()];
                        key.extend(ctx.path().to_owned());
                        self.1.insert(key, opt.clone());
                    }
                }
                Ok(())
            }
        }

        cfg.visit(
            &mut VisitorContext::new(),
            &mut ResolveNetRefVisitor(prefix, &mut to_add),
        )?;

        Ok(to_add)
    }
    pub fn build(&self, getter: NetGetter, cfg: Value) -> Result<ItemType> {
        (self.build)(getter, cfg)
    }
    pub fn get_dependency(&self, cfg: Value) -> Result<Vec<String>> {
        let mut cfg = (self.parse_config)(cfg)?;

        struct GetDependencyVisitor<'a>(&'a mut HashSet<String>);

        impl<'a> Visitor for GetDependencyVisitor<'a> {
            fn visit_net_ref(
                &mut self,
                _ctx: &mut VisitorContext,
                net_ref: &mut NetRef,
            ) -> Result<()> {
                if let Some(name) = net_ref.represent().as_str() {
                    self.0.insert(name.to_string());
                }
                Ok(())
            }
        }

        let mut nets = HashSet::new();
        cfg.visit(
            &mut VisitorContext::new(),
            &mut GetDependencyVisitor(&mut nets),
        )?;

        Ok(nets.into_iter().collect())
    }
    pub fn schema(&self) -> &RootSchema {
        &self.schema
    }
}
