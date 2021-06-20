use std::{collections::BTreeMap, fmt};

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
    pub nets: BTreeMap<String, Net>,
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
            move |net_name: String, net: Net| c.get_net(net_name, net)
        };
        let wrap_server_net = {
            let c = ctl.clone();
            move |net: Net| c.get_server_net(net)
        };
        let mut registry = Registry::new();

        load_builtin(&mut registry).context("Failed to load builtin")?;
        (self.plugin_loader)(&config, &mut registry).context("Failed to load plugin")?;
        tracing::debug!("Registry:\n{}", registry);

        let all_net = config
            .net
            .iter()
            .map(|(k, v)| (k.to_string(), AllNet::Net(v.clone())))
            .collect();
        let nets = build_net(&registry, all_net, &config.server, wrap_net)
            .context("Failed to build net")?;
        let servers = build_server(&registry, &nets, &config.server, wrap_server_net)
            .context("Failed to build server")?;
        tracing::debug!(
            "net and server are built. net count: {}, server count: {}",
            nets.len(),
            servers.len()
        );

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
    mut all_net: BTreeMap<String, config::AllNet>,
    server: &config::ConfigServer,
    wrapper: impl Fn(String, Net) -> Net,
) -> Result<BTreeMap<String, Net>> {
    let mut net_map: BTreeMap<String, Net> = BTreeMap::new();

    if !all_net.contains_key("noop") {
        all_net.insert(
            "noop".to_string(),
            AllNet::Net(config::Net::new("noop", Value::Object(Map::new()))),
        );
    }
    if !all_net.contains_key("local") {
        all_net.insert(
            "local".to_string(),
            AllNet::Net(config::Net::new("local", Value::Object(Map::new()))),
        );
    }
    all_net.insert(
        "_".to_string(),
        AllNet::Root(
            server
                .values()
                .flat_map(|i| vec![i.net.clone(), i.listen.clone()])
                .collect(),
        ),
    );

    let all_net = topological_sort(all_net, |k, n| {
        n.get_dependency(registry)
            .context(format!("Failed to get_dependency for net/server: {}", k))
    })
    .context("Failed to do topological_sort")?
    .ok_or(anyhow!("There is cyclic dependencies in net",))?;

    for (name, i) in all_net {
        match i {
            AllNet::Net(i) => {
                let load_net = || -> Result<()> {
                    let net_item = registry.get_net(&i.net_type)?;

                    let net = net_item.build(&net_map, i.opt).context(format!(
                        "Failed to build net {:?}. Please check your config.",
                        name
                    ))?;
                    let net = wrapper(name.to_string(), net);
                    net_map.insert(name.to_string(), net);
                    Ok(())
                };
                load_net().context(format!("Loading net {}", name))?;
            }
            AllNet::Root(_) => {}
        }
    }

    Ok(net_map)
}

fn build_server(
    registry: &Registry,
    net: &BTreeMap<String, Net>,
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
        load_server().context(format!("Loading server {}", name))?;
    }

    Ok(servers)
}
