use std::{collections::HashMap, fmt};

use serde::de::DeserializeOwned;

use crate::{config::Value, INet, IServer, IntoDyn, Net, Result, Server};

pub type NetFromConfig<T> = Box<dyn Fn(Vec<Net>, Value) -> Result<T>>;
/// listen_net, net, config
pub type ServerFromConfig<T> = Box<dyn Fn(Net, Net, Value) -> Result<T>>;

pub struct Registry {
    pub net: HashMap<String, NetFromConfig<Net>>,
    pub server: HashMap<String, ServerFromConfig<Server>>,
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
        self.net.insert(N::NAME.into(), N::into_dyn());
    }
    pub fn add_server<S: IServer + 'static>(
        &mut self,
        name: impl Into<String>,
        from_cfg: impl Fn(Net, Net, Value) -> Result<S> + 'static,
    ) {
        self.server.insert(
            name.into(),
            Box::new(move |listen_net, net, cfg| {
                from_cfg(listen_net, net, cfg).map(|n| n.into_dyn())
            }),
        );
    }
}

pub trait NetFactory: INet + Sized + 'static {
    const NAME: &'static str;
    type Config: DeserializeOwned;

    fn new(nets: Vec<Net>, config: Self::Config) -> Result<Self>;
    fn into_dyn() -> NetFromConfig<Net>
    where
        Self: Sized + 'static,
    {
        Box::new(move |net, cfg| {
            serde_json::from_value(cfg)
                .map_err(Into::<crate::Error>::into)
                .and_then(|cfg| Self::new(net, cfg))
                .map(|n| n.into_dyn())
        })
    }
}
