use std::{collections::HashMap, fmt};

use crate::builtin::load_builtin;
use crate::config;
use crate::controller;
use crate::registry::Registry;
use crate::util::topological_sort;
use anyhow::{anyhow, Context, Result};
use config::AllNet;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use rd_interface::{Arc, Net, Server, Value};
use serde_json::Map;

pub type PluginLoader =
    Arc<dyn Fn(&config::Config, &mut Registry) -> Result<()> + Send + Sync + 'static>;

#[derive(Clone)]
pub struct RabbitDiggerBuilder {
    pub plugin_loader: PluginLoader,
}

pub struct RabbitDigger {
    pub config: config::Config,
    pub registry: Registry,
    pub nets: HashMap<String, Net>,
    pub servers: Vec<ServerInfo>,
}

impl fmt::Debug for RabbitDigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RabbitDigger").finish()
    }
}

impl RabbitDigger {
    pub async fn run(servers: Vec<ServerInfo>) -> Result<()> {
        tracing::info!("Server:\n{}", ServerList(&servers));

        let mut server_tasks: FuturesUnordered<_> = servers
            .iter()
            .map(|i| {
                let name = i.name.clone();
                start_server(&i.server).map(|r| (name, r)).boxed()
            })
            .collect();

        while let Some((name, r)) = server_tasks.next().await {
            tracing::info!("Server {} is stopped. Return: {:?}", name, r)
        }

        tracing::info!("all servers are down, exit.");
        Ok(())
    }
}

impl RabbitDiggerBuilder {
    pub fn new() -> RabbitDiggerBuilder {
        RabbitDiggerBuilder {
            plugin_loader: Arc::new(|_, _| Ok(())),
        }
    }
    pub fn build(
        &self,
        ctl: &controller::Controller,
        config: config::Config,
    ) -> Result<RabbitDigger> {
        let wrap_net = {
            let c = ctl.clone();
            move |net: Net| c.get_net(net)
        };
        let mut registry = Registry::new();

        load_builtin(&mut registry)?;
        (self.plugin_loader)(&config, &mut registry)?;
        tracing::debug!("Registry:\n{}", registry);

        let all_net = config
            .net
            .iter()
            .map(|(k, v)| (k.to_string(), AllNet::Net(v.clone())))
            .collect();
        let nets = build_net(&registry, all_net, &config.server)?;
        let servers = build_server(&registry, &nets, &config.server, wrap_net)?;

        Ok(RabbitDigger {
            config,
            registry,
            nets,
            servers,
        })
    }
}

async fn start_server(server: &Server) -> Result<()> {
    server.start().await?;
    Ok(())
}

pub struct ServerInfo {
    name: String,
    listen: String,
    net: String,
    server: Server,
    config: Value,
}

struct ServerList<'a>(&'a Vec<ServerInfo>);

impl fmt::Display for ServerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} -> {} {}",
            self.name, self.listen, self.net, self.config
        )
    }
}

impl<'a> fmt::Display for ServerList<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in self.0.iter() {
            writeln!(f, "\t{}", i)?;
        }
        Ok(())
    }
}

fn build_net(
    registry: &Registry,
    mut all_net: HashMap<String, config::AllNet>,
    server: &config::ConfigServer,
) -> Result<HashMap<String, Net>> {
    let mut net: HashMap<String, Net> = HashMap::new();

    if !all_net.contains_key("noop") {
        all_net.insert(
            "noop".to_string(),
            AllNet::Net(config::Net {
                net_type: "noop".to_string(),
                opt: Value::Object(Map::new()),
            }),
        );
    }
    if !all_net.contains_key("local") {
        all_net.insert(
            "local".to_string(),
            AllNet::Net(config::Net {
                net_type: "local".to_string(),
                opt: Value::Object(Map::new()),
            }),
        );
    }
    all_net.insert(
        "_".to_string(),
        AllNet::Root(server.values().map(|i| i.net.clone()).collect()),
    );

    let all_net = topological_sort(all_net, |n| n.get_dependency(registry))?
        .ok_or(anyhow!("There is cyclic dependencies in net",))?;

    for (name, i) in all_net {
        match i {
            AllNet::Net(i) => {
                let load_net = || -> Result<()> {
                    let net_item = registry.get_net(&i.net_type)?;

                    let proxy = net_item.build(&net, i.opt).context(format!(
                        "Failed to build net {:?}. Please check your config.",
                        name
                    ))?;
                    net.insert(name.to_string(), proxy);
                    Ok(())
                };
                load_net().map_err(|e| e.context(format!("Loading net {}", name)))?;
            }
            AllNet::Root(_) => {}
        }
    }

    Ok(net)
}

fn build_server(
    registry: &Registry,
    net: &HashMap<String, Net>,
    config: &config::ConfigServer,
    wrapper: impl Fn(Net) -> Net,
) -> Result<Vec<ServerInfo>> {
    let mut servers: Vec<ServerInfo> = Vec::new();
    let config = config.clone();

    for (name, i) in config {
        let name = &name;
        let load_server = || -> Result<()> {
            let server_item = registry.get_server(&i.server_type)?;
            let listen = net.get(&i.listen).ok_or(anyhow!(
                "Listen Net {} is not loaded. Required by {:?}",
                &i.net,
                &name
            ))?;
            let net = net.get(&i.net).ok_or(anyhow!(
                "Net {} is not loaded. Required by {:?}",
                &i.net,
                &name
            ))?;

            let server = server_item
                .build(listen.clone(), wrapper(net.clone()), i.opt.clone())
                .context(format!(
                    "Failed to build server {:?}. Please check your config.",
                    name
                ))?;
            servers.push(ServerInfo {
                name: name.to_string(),
                server,
                config: i.opt,
                listen: i.listen,
                net: i.net,
            });
            Ok(())
        };
        load_server().map_err(|e| e.context(format!("Loading server {}", name)))?;
    }

    Ok(servers)
}
