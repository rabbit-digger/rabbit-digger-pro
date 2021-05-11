use std::{collections::HashMap, fmt};

use crate::{INet, IServer, IntoDyn, Net, Result, Server};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub type FromConfig<T> = Box<dyn Fn(Vec<Net>, Value) -> Result<T>>;

pub struct Registry {
    pub net: HashMap<String, FromConfig<Net>>,
    pub server: HashMap<String, FromConfig<Server>>,
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
    pub fn add_server<S: ServerFactory>(&mut self) {
        self.server.insert(S::NAME.into(), S::into_dyn());
    }
}

pub trait NetFactory: INet + Sized + 'static {
    const NAME: &'static str;
    type Config: DeserializeOwned;

    fn new(nets: Vec<Net>, config: Self::Config) -> Result<Self>;
    fn into_dyn() -> FromConfig<Net>
    where
        Self: Sized + 'static,
    {
        Box::new(move |nets, cfg| {
            serde_json::from_value(cfg)
                .map_err(Into::<crate::Error>::into)
                .and_then(|cfg| Self::new(nets, cfg))
                .map(|n| n.into_dyn())
        })
    }
}

pub trait ServerFactory: IServer + Sized + 'static {
    const NAME: &'static str;
    type Config: DeserializeOwned;

    fn new(listen_net: Net, net: Net, config: Self::Config) -> Result<Self>;
    fn into_dyn() -> FromConfig<Server>
    where
        Self: Sized + 'static,
    {
        Box::new(move |mut nets, cfg| {
            serde_json::from_value(cfg)
                .map_err(Into::<crate::Error>::into)
                .and_then(|cfg| {
                    if nets.len() != 2 {
                        return Err(crate::Error::Other(
                            "Server must have listen_net and net".to_string().into(),
                        ));
                    }
                    let listen_net = nets.remove(0);
                    let net = nets.remove(0);

                    Self::new(listen_net, net, cfg)
                })
                .map(|n| n.into_dyn())
        })
    }
}
