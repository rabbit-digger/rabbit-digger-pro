use std::{collections::HashMap, fmt, path::PathBuf, time::Duration};

use crate::composite;
use crate::config;
use crate::controller;
use crate::plugins::load_plugins;
use crate::registry::Registry;
use anyhow::{anyhow, Context, Result};
use async_std::{
    fs::{read_to_string, File},
    future::timeout,
};
use config::AllNet;
use futures::{
    future::{ready, try_select, Either},
    pin_mut,
    prelude::*,
    stream::{self, FuturesUnordered, Stream},
    StreamExt,
};
use notify_stream::{notify::RecursiveMode, notify_stream};
use rd_interface::{config::Value, Arc, ConnectionPool, Net, NotImplementedNet, Server};
use topological_sort::TopologicalSort;

pub type PluginLoader = Box<dyn Fn(&config::Config) -> Result<Registry> + 'static>;
pub struct RabbitDigger {
    config_path: PathBuf,
    plugin_loader: PluginLoader,
    pub write_config: Option<PathBuf>,
}

impl RabbitDigger {
    pub fn new(config_path: PathBuf) -> Result<RabbitDigger> {
        Ok(RabbitDigger {
            config_path,
            plugin_loader: Box::new(|cfg| load_plugins(cfg.plugin_path.clone())),
            write_config: None,
        })
    }
    pub fn with_plugin_loader(
        config_path: PathBuf,
        plugin_loader: impl Fn(&config::Config) -> Result<Registry> + 'static,
    ) -> Result<RabbitDigger> {
        Ok(RabbitDigger {
            config_path,
            plugin_loader: Box::new(plugin_loader),
            write_config: None,
        })
    }
    pub async fn run(&self, controller: &controller::Controller) -> Result<()> {
        let config_path = self.config_path.clone();
        let config_stream = notify_stream(&config_path, RecursiveMode::Recursive)?;

        let config_stream = stream::once(async { Ok(()) })
            .chain(async_std::stream::StreamExt::delay(
                config_stream
                    .try_filter(|e| ready(e.kind.is_modify()))
                    .map_err(Into::<anyhow::Error>::into)
                    .map(|_| Ok(())),
                Duration::from_millis(100),
            ))
            .and_then(move |_| read_to_string(config_path.clone()).map_err(Into::into))
            .and_then(|s| ready(serde_yaml::from_str(&s).map_err(Into::into)))
            .and_then(|c: config::Config| c.post_process());
        pin_mut!(config_stream);
        self._run(controller, config_stream).await
    }

    async fn _run<S>(&self, controller: &controller::Controller, mut config_stream: S) -> Result<()>
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
            if let Some(path) = &self.write_config {
                File::create(path)
                    .await?
                    .write_all(&serde_yaml::to_vec(&config)?)
                    .await?;
            }
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
            config = match new_config {
                Some(v) => v,
                None => return Ok(()),
            }
        }
    }

    async fn run_once(&self, ctl: &controller::Controller, config: config::Config) -> Result<()> {
        let wrap_net = {
            let c = ctl.clone();
            move |net: Net| c.get_net(net)
        };
        let registry = (self.plugin_loader)(&config)?;
        log::debug!("registry: {:?}", registry);

        let net_cfg = config.net.into_iter().map(|(k, v)| (k, AllNet::Net(v)));
        let composite_cfg = config
            .composite
            .into_iter()
            .map(|(k, v)| (k, AllNet::Composite(v)));
        let all_net = net_cfg.chain(composite_cfg).collect();
        let net = init_net(&registry, all_net)?;
        let servers = init_server(&registry, &net, config.server, wrap_net)?;

        log::info!("server {:#?}", servers);

        let pool = ConnectionPool::new()?;
        let mut tasks: FuturesUnordered<_> = servers
            .into_iter()
            .map(|i| start_server(i.server, pool.clone()).boxed())
            .collect();

        while tasks.next().await.is_some() {}

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

impl fmt::Debug for ServerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerInfo")
            .field("name", &self.name)
            .field("listen", &self.listen)
            .field("net", &self.net)
            .field("config", &self.config)
            .finish()
    }
}

fn init_net(
    registry: &Registry,
    mut all_net: HashMap<String, config::AllNet>,
) -> Result<HashMap<String, Net>> {
    let mut net: HashMap<String, Net> = HashMap::new();
    let noop = Arc::new(NotImplementedNet);

    all_net.insert("noop".to_string(), AllNet::Noop);
    all_net.insert("local".to_string(), AllNet::Local);

    let mut ts = TopologicalSort::<String>::new();
    for (k, n) in all_net.iter() {
        for d in n.get_dependency()? {
            ts.add_dependency(d, k.clone());
        }
    }

    while let Some(name) = ts.pop() {
        match all_net
            .remove(&name)
            .ok_or(anyhow!("Failed to get net by name: {}", name))?
        {
            AllNet::Net(i) => {
                let load_net = || -> Result<()> {
                    let net_item = registry.get_net(&i.net_type)?;
                    let chains = i
                        .chain
                        .to_vec()
                        .into_iter()
                        .map(|s| {
                            net.get(&s).map(|s| s.clone()).ok_or(anyhow!(
                                "Chain {} is not loaded. Required by {}",
                                &s,
                                name
                            ))
                        })
                        .collect::<Result<Vec<_>>>()?;

                    let proxy = net_item.build(chains, i.rest).context(format!(
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
                    composite::build_composite(net.clone(), i)
                        .context(format!("Loading composite {}", name))?,
                );
            }
            AllNet::Local => {
                net.insert(name, get_local(&registry)?);
            }
            AllNet::Noop => {
                net.insert(name, noop.clone());
            }
        }
    }

    if ts.len() == 0 {
        Ok(net)
    } else {
        Err(anyhow!("There is cyclic dependencies in net",))
    }
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
            log::trace!("Loading server: {}", name);
            let server = server_item
                .build(listen.clone(), wrapper(net.clone()), i.rest.clone())
                .context(format!(
                    "Failed to build server {:?}. Please check your config.",
                    name
                ))?;
            servers.push(ServerInfo {
                name: name.to_string(),
                server,
                config: i.rest,
                listen: i.listen,
                net: i.net,
            });
            Ok(())
        };
        load_server().map_err(|e| e.context(format!("Loading server {}", name)))?;
    }

    Ok(servers)
}
