//! A registry with plugin name

use anyhow::{anyhow, Result};
use rd_interface::{registry::FromConfig, Net, Server, Value};
use std::{collections::HashMap, fmt};

pub struct NetItem {
    pub plugin_name: String,
    pub factory: FromConfig<Net>,
}

pub struct ServerItem {
    pub plugin_name: String,
    pub factory: FromConfig<Server>,
}

impl NetItem {
    pub fn build(&self, nets: Vec<Net>, config: Value) -> rd_interface::Result<Net> {
        (self.factory)(nets, config)
    }
}

impl ServerItem {
    pub fn build(&self, listen_net: Net, net: Net, config: Value) -> rd_interface::Result<Server> {
        (self.factory)(vec![listen_net, net], config)
    }
}

pub struct Registry {
    pub net: HashMap<String, NetItem>,
    pub server: HashMap<String, ServerItem>,
}

impl fmt::Debug for NetItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetItem")
            .field("plugin_name", &self.plugin_name)
            .finish()
    }
}

impl fmt::Debug for ServerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerItem")
            .field("plugin_name", &self.plugin_name)
            .finish()
    }
}

impl fmt::Display for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Net")?;
        for (k, v) in self.net.iter() {
            writeln!(f, "\t{}: {}", k, v.plugin_name)?;
        }
        writeln!(f, "Server")?;
        for (k, v) in self.server.iter() {
            writeln!(f, "\t{}: {}", k, v.plugin_name)?;
        }
        Ok(())
    }
}

impl Registry {
    pub fn new() -> Registry {
        Registry {
            net: HashMap::new(),
            server: HashMap::new(),
        }
    }
    pub fn init_with_registry(
        &mut self,
        name: impl Into<String>,
        init: impl Fn(&mut rd_interface::Registry) -> Result<()>,
    ) -> Result<()> {
        let mut r = rd_interface::Registry::new();
        init(&mut r)?;
        self.add_registry(name.into(), r);
        Ok(())
    }
    fn add_registry(&mut self, plugin_name: String, registry: rd_interface::Registry) {
        for (k, v) in registry.net {
            self.net.insert(
                k,
                NetItem {
                    plugin_name: plugin_name.clone(),
                    factory: v,
                },
            );
        }
        for (k, v) in registry.server {
            self.server.insert(
                k,
                ServerItem {
                    plugin_name: plugin_name.clone(),
                    factory: v,
                },
            );
        }
    }
    pub fn get_net(&self, net_type: &str) -> Result<&NetItem> {
        self.net
            .get(net_type)
            .ok_or(anyhow!("Net type is not loaded: {}", net_type))
    }
    pub fn get_server(&self, server_type: &str) -> Result<&ServerItem> {
        self.server
            .get(server_type)
            .ok_or(anyhow!("Server type is not loaded: {}", server_type))
    }
}
