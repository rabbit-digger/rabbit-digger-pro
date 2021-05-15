use std::{collections::HashMap, fmt};

pub use self::net_ref::{NetRef, ResolveNetRef};
use crate::{INet, IServer, IntoDyn, Net, Result, Server};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub type NetMap = HashMap<String, Net>;

mod net_ref;

pub struct Registry {
    pub net: HashMap<String, NetResolver>,
    pub server: HashMap<String, ServerResolver>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("net", &self.net.keys())
            .field("server", &self.server.keys())
            .finish()
    }
}

impl Registry {
    pub fn new() -> Registry {
        Registry {
            net: HashMap::new(),
            server: HashMap::new(),
        }
    }
    pub fn add_net<N: NetFactory>(&mut self) {
        self.net.insert(N::NAME.into(), NetResolver::new::<N>());
    }
    pub fn add_server<S: ServerFactory>(&mut self) {
        self.server
            .insert(S::NAME.into(), ServerResolver::new::<S>());
    }
}

pub trait NetFactory {
    const NAME: &'static str;
    type Config: DeserializeOwned + ResolveNetRef;
    type Net: INet + Sized + 'static;

    fn new(config: Self::Config) -> Result<Self::Net>;
}

pub struct NetResolver {
    build: fn(nets: &NetMap, cfg: Value) -> Result<Net>,
    get_dependency: fn(cfg: Value) -> Result<Vec<String>>,
}

impl NetResolver {
    fn new<N: NetFactory>() -> Self {
        Self {
            build: |nets, cfg| {
                serde_json::from_value(cfg)
                    .map_err(Into::<crate::Error>::into)
                    .and_then(|mut cfg: N::Config| {
                        cfg.resolve(nets)?;
                        Ok(cfg)
                    })
                    .and_then(|cfg| N::new(cfg))
                    .map(|n| n.into_dyn())
            },
            get_dependency: |cfg| {
                serde_json::from_value(cfg)
                    .map_err(Into::<crate::Error>::into)
                    .and_then(|mut cfg: N::Config| cfg.get_dependency())
            },
        }
    }
    pub fn build(&self, nets: &NetMap, cfg: Value) -> Result<Net> {
        (self.build)(nets, cfg)
    }
    pub fn get_dependency(&self, cfg: Value) -> Result<Vec<String>> {
        (self.get_dependency)(cfg)
    }
}

pub trait ServerFactory {
    const NAME: &'static str;
    type Config: DeserializeOwned;
    type Server: IServer + Sized + 'static;

    fn new(listen: Net, net: Net, config: Self::Config) -> Result<Self::Server>;
}

pub struct ServerResolver {
    build: fn(listen_net: Net, net: Net, cfg: Value) -> Result<Server>,
}

impl ServerResolver {
    fn new<N: ServerFactory>() -> Self {
        Self {
            build: |listen_net, net: Net, cfg| {
                serde_json::from_value(cfg)
                    .map_err(Into::<crate::Error>::into)
                    .and_then(|cfg| N::new(listen_net, net, cfg))
                    .map(|n| n.into_dyn())
            },
        }
    }
    pub fn build(&self, listen_net: Net, net: Net, cfg: Value) -> Result<Server> {
        (self.build)(listen_net, net, cfg)
    }
}

#[derive(Debug, Default, serde_derive::Deserialize)]
pub struct EmptyConfig(Value);

impl ResolveNetRef for EmptyConfig {
    fn resolve(&mut self, _nets: &NetMap) -> Result<()> {
        Ok(())
    }
}
