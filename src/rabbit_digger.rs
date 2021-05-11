use std::{collections::HashMap, fmt, future::ready, time::Duration};

use crate::config;
use crate::controller;
use crate::registry::Registry;
use crate::util::topological_sort;
use crate::{builtin::load_builtin, composite};
use anyhow::{anyhow, Context, Result};
use async_std::future::timeout;
use config::AllNet;
use futures::{
    future::{try_select, Either},
    pin_mut,
    prelude::*,
    stream::{self, FuturesUnordered, Stream},
    StreamExt,
};
use rd_interface::{Arc, ConnectionPool, Net, NotImplementedNet, Server, Value};

pub type PluginLoader = Box<dyn Fn(&config::Config, &mut Registry) -> Result<()> + 'static>;
pub struct RabbitDigger {
    plugin_loader: PluginLoader,
}

impl RabbitDigger {
    pub fn new() -> Result<RabbitDigger> {
        Ok(RabbitDigger {
            plugin_loader: Box::new(|_, _| Ok(())),
        })
    }
    pub async fn run(
        &self,
        controller: &controller::Controller,
        config: config::Config,
    ) -> Result<()> {
        let config_stream = stream::once(ready(Ok(config)));
        self.run_stream(controller, config_stream).await
    }
    pub async fn run_stream<S>(
        &self,
        controller: &controller::Controller,
        mut config_stream: S,
    ) -> Result<()>
    where
        S: Stream<Item = Result<config::Config>> + Unpin,
    {
        let mut config = match timeout(Duration::from_secs(1), config_stream.try_next()).await {
            Ok(Ok(Some(cfg))) => cfg,
            Ok(Err(e)) => return Err(e.context(format!("Failed to get first config."))),
            Err(_) | Ok(Ok(None)) => {
                return Err(anyhow!("The config_stream is empty, can not start."))
            }
        };
        let mut config_stream = config_stream.chain(stream::pending());

        loop {
            log::info!("rabbit digger is starting...");

            controller.update_config(config.clone()).await?;
            let run_fut = self.run_once(controller, config);
            pin_mut!(run_fut);
            let new_config = match try_select(run_fut, config_stream.try_next()).await {
                Ok(Either::Left((_, cfg_fut))) => {
                    log::info!("Exited normally, waiting for next config...");
                    cfg_fut.await?
                }
                Ok(Either::Right((cfg, _))) => cfg,
                Err(Either::Left((e, cfg_fut))) => {
                    log::error!("Error: {:?}, waiting for next config...", e);
                    cfg_fut.await?
                }
                Err(Either::Right((e, _))) => return Err(e),
            };
            controller.remove_config().await?;

            config = match new_config {
                Some(v) => v,
                None => return Ok(()),
            }
        }
    }

    pub async fn run_once(
        &self,
        ctl: &controller::Controller,
        config: config::Config,
    ) -> Result<()> {
        let wrap_net = {
            let c = ctl.clone();
            move |net: Net| c.get_net(net)
        };
        let mut registry = Registry::new();

        load_builtin(&mut registry)?;
        (self.plugin_loader)(&config, &mut registry)?;
        log::debug!("Registry:\n{}", registry);

        let net_cfg = config.net.into_iter().map(|(k, v)| (k, AllNet::Net(v)));
        let composite_cfg = config
            .composite
            .into_iter()
            .map(|(k, v)| (k, AllNet::Composite(v)));
        let all_net = net_cfg.chain(composite_cfg).collect();
        let net = init_net(&registry, all_net, &config.server)?;
        let servers = init_server(&registry, &net, config.server, wrap_net)?;

        log::info!("Server:\n{}", ServerList(&servers));

        let pool = ConnectionPool::new()?;
        let mut server_tasks: FuturesUnordered<_> = servers
            .into_iter()
            .map(|i| {
                let name = i.name;
                start_server(i.server, pool.clone())
                    .map(|r| (name, r))
                    .boxed()
            })
            .collect();

        while let Some((name, r)) = server_tasks.next().await {
            log::info!("Server {} is stopped. Return: {:?}", name, r)
        }

        log::info!("all servers are down, exit.");

        Ok(())
    }
}

fn get_local(registry: &Registry) -> Result<Net> {
    let net_item = &registry.get_net("local")?;
    let local = net_item.build(Vec::new(), Value::Null)?;
    Ok(local)
}

async fn start_server(server: Server, pool: ConnectionPool) -> Result<()> {
    server.start(pool).await?;
    Ok(())
}

struct ServerInfo {
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

fn clone_net_by_net_list(
    net: &HashMap<String, Net>,
    net_list: &Vec<String>,
) -> Result<HashMap<String, Net>> {
    if net_list.len() == 0 {
        return Ok(net.clone());
    }
    net_list
        .into_iter()
        .map(|target| {
            Ok((
                target.clone(),
                net.get(target)
                    .ok_or(anyhow!("target is not found: {}", target))?
                    .to_owned(),
            ))
        })
        .collect::<Result<HashMap<_, _>>>()
}

fn init_net(
    registry: &Registry,
    mut all_net: HashMap<String, config::AllNet>,
    server: &config::ConfigServer,
) -> Result<HashMap<String, Net>> {
    let mut net: HashMap<String, Net> = HashMap::new();
    let noop = Arc::new(NotImplementedNet);

    all_net.insert("noop".to_string(), AllNet::Noop);
    all_net.insert("local".to_string(), AllNet::Local);
    all_net.insert(
        "_".to_string(),
        AllNet::Root(server.values().map(|i| i.net.clone()).collect()),
    );

    let all_net = topological_sort(all_net, AllNet::get_dependency)
        .ok_or(anyhow!("There is cyclic dependencies in net",))?;

    for (name, i) in all_net {
        match i {
            AllNet::Net(i) => {
                let load_net = || -> Result<()> {
                    let net_item = registry.get_net(&i.net_type)?;
                    let chains = i
                        .chain
                        .into_iter()
                        .map(|s| {
                            net.get(&s).map(|s| s.clone()).ok_or(anyhow!(
                                "Chain {} is not loaded. Required by {}",
                                &s,
                                name
                            ))
                        })
                        .collect::<Result<Vec<_>>>()?;

                    let proxy = net_item.build(chains, i.opt).context(format!(
                        "Failed to build net {:?}. Please check your config.",
                        name
                    ))?;
                    net.insert(name.to_string(), proxy);
                    Ok(())
                };
                load_net().map_err(|e| e.context(format!("Loading net {}", name)))?;
            }
            AllNet::Composite(i) => {
                net.insert(
                    name.to_string(),
                    composite::build_composite(clone_net_by_net_list(&net, &i.net_list)?, i)
                        .context(format!("Loading composite {}", name))?,
                );
            }
            AllNet::Local => {
                net.insert(name, get_local(&registry)?);
            }
            AllNet::Noop => {
                net.insert(name, noop.clone());
            }
            AllNet::Root(_) => {}
        }
    }

    Ok(net)
}

fn init_server(
    registry: &Registry,
    net: &HashMap<String, Net>,
    config: config::ConfigServer,
    wrapper: impl Fn(Net) -> Net,
) -> Result<Vec<ServerInfo>> {
    let mut servers: Vec<ServerInfo> = Vec::new();

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
