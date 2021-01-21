use std::{collections::HashMap, fmt, sync::Arc};

use crate::{config::Value, INet, IServer, Net, Result, Server};

pub type NetFromConfig<T> = Box<dyn Fn(Net, Value) -> Result<T>>;

pub struct Registry {
    pub net: HashMap<String, NetFromConfig<Net>>,
    pub server: HashMap<String, NetFromConfig<Server>>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("net", &self.net.keys())
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
    pub fn add_net<N: INet + 'static>(
        &mut self,
        name: impl Into<String>,
        from_cfg: impl Fn(Net, Value) -> Result<N> + 'static,
    ) {
        self.net.insert(
            name.into(),
            Box::new(move |net, cfg| {
                from_cfg(net, cfg).map(|n| Arc::new(n) as Arc<(dyn INet + 'static)>)
            }),
        );
    }
    pub fn add_server<S: IServer + 'static>(
        &mut self,
        name: impl Into<String>,
        from_cfg: impl Fn(Net, Value) -> Result<S> + 'static,
    ) {
        self.server.insert(
            name.into(),
            Box::new(move |net, cfg| {
                from_cfg(net, cfg).map(|n| Box::new(n) as Box<dyn IServer + 'static>)
            }),
        );
    }
}
