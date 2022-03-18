use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

pub use crate::config::NetRef;
use crate::{
    config::{resolve_net, CompactVecString, Config, NetSchema, Visitor, VisitorContext},
    IntoDyn, Net, Result, Server,
};
pub use schemars::JsonSchema;
use schemars::{schema::RootSchema, schema_for};
use serde::{de::DeserializeOwned, Serialize};
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
    type Config: Serialize + DeserializeOwned + JsonSchema + Config + 'static;
    type Item: IntoDyn<ItemType> + Sized + 'static;

    fn build(config: Self::Config) -> Result<Self::Item>;
}

impl<ItemType, T: Builder<ItemType>> BuilderExt<ItemType> for T {}
trait BuilderExt<ItemType>: Builder<ItemType> {
    fn get_dependency(cfg: Value) -> Result<Vec<String>> {
        let mut cfg =
            serde_json::from_value::<Self::Config>(cfg).map_err(Into::<crate::Error>::into)?;

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

    fn unfold_net_ref(
        cfg_value: &mut Value,
        prefix: &[&str],
        delimiter: &str,
        add_net: &mut HashMap<CompactVecString, Value>,
    ) -> Result<()> {
        let mut cfg = serde_json::from_value::<Self::Config>(cfg_value.clone())
            .map_err(Into::<crate::Error>::into)?;
        struct ResolveNetRefVisitor<'a> {
            prefix: &'a [&'a str],
            delimiter: &'a str,
            to_add: &'a mut HashMap<CompactVecString, Value>,
        }

        impl<'a> Visitor for ResolveNetRefVisitor<'a> {
            fn visit_net_ref(
                &mut self,
                ctx: &mut VisitorContext,
                net_ref: &mut crate::config::Resolvable<NetSchema>,
            ) -> Result<()> {
                match net_ref.represent() {
                    Value::String(_) => {}
                    opt => {
                        let opt = opt.clone();
                        let mut key = self.prefix.iter().map(|i| *i).collect::<CompactVecString>();
                        key.extend(ctx.path());
                        *net_ref.represent_mut() = Value::String(key.join(self.delimiter));
                        self.to_add.insert(key, opt);
                    }
                }
                Ok(())
            }
        }

        cfg.visit(
            &mut VisitorContext::new(),
            &mut ResolveNetRefVisitor {
                prefix,
                delimiter,
                to_add: add_net,
            },
        )?;
        *cfg_value = serde_json::to_value(cfg)?;

        Ok(())
    }

    fn build_dyn(getter: NetGetter, cfg: Value) -> Result<ItemType> {
        serde_json::from_value(cfg)
            .map_err(Into::<crate::Error>::into)
            .and_then(|mut cfg: Self::Config| {
                resolve_net(&mut cfg, getter)?;
                Ok(cfg)
            })
            .and_then(Self::build)
            .map(|n| n.into_dyn())
    }
}

pub struct Resolver<ItemType> {
    get_dependency: fn(cfg: Value) -> Result<Vec<String>>,
    unfold_net_ref: fn(
        cfg: &mut Value,
        prefix: &[&str],
        delimiter: &str,
        add_net: &mut HashMap<CompactVecString, Value>,
    ) -> Result<()>,
    build: fn(getter: NetGetter, cfg: Value) -> Result<ItemType>,
    schema: RootSchema,
}
pub type NetResolver = Resolver<Net>;
pub type ServerResolver = Resolver<Server>;

impl<ItemType> Resolver<ItemType> {
    pub fn new<N: Builder<ItemType>>() -> Self {
        let schema = schema_for!(N::Config);
        Self {
            get_dependency: N::get_dependency,
            unfold_net_ref: N::unfold_net_ref,
            build: N::build_dyn,
            schema,
        }
    }
    pub fn unfold_net_ref(
        &self,
        cfg: &mut Value,
        prefix: &[&str],
        delimiter: &str,
        add_net: &mut HashMap<CompactVecString, Value>,
    ) -> Result<()> {
        (self.unfold_net_ref)(cfg, prefix, delimiter, add_net)
    }
    pub fn build(&self, getter: NetGetter, cfg: Value) -> Result<ItemType> {
        (self.build)(getter, cfg)
    }
    pub fn get_dependency(&self, cfg: Value) -> Result<Vec<String>> {
        (self.get_dependency)(cfg)
    }
    pub fn schema(&self) -> &RootSchema {
        &self.schema
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_debug() {
        let reg = Registry::default();
        assert_eq!(format!("{:?}", reg), "Registry { net: [], server: [] }");
    }
}
