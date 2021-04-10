use std::{collections::HashMap, fmt, path::PathBuf, time::Duration};

use crate::config;
use crate::controller;
use crate::plugins::load_plugins;
use crate::registry::Registry;
use crate::rule::Rule;
use anyhow::{anyhow, Context, Result};
use async_std::{fs::read_to_string, future::timeout};
use futures::{
    future::{ready, try_select, Either},
    pin_mut,
    prelude::*,
    stream::{self, FuturesUnordered, Stream},
    StreamExt,
};
use notify_stream::{notify::RecursiveMode, notify_stream, NotifyStream};
use rd_interface::{config::Value, Arc, Net, NotImplementedNet, Server};

pub struct RabbitDigger {
    config_stream: NotifyStream,
    config_path: PathBuf,
}

pub async fn run<S>(controller: &controller::Controller, mut config_stream: S) -> Result<()>
where
    S: Stream<Item = Result<config::Config>> + Unpin,
{
    let mut config = match timeout(Duration::from_secs(1), config_stream.try_next()).await {
        Ok(Ok(Some(cfg))) => cfg,
        Ok(Err(e)) => return Err(anyhow!("Failed to get first config {}.", e)),
        Err(_) | Ok(Ok(None)) => return Err(anyhow!("The config_stream is empty, can not start.")),
    };
    let mut config_stream = config_stream.chain(stream::pending());

    loop {
        log::info!("rabbit digger is starting...");
        let run_fut = run_once(controller, config);
        pin_mut!(run_fut);
        let new_config = match try_select(run_fut, config_stream.try_next()).await {
            Ok(Either::Left((_, cfg_fut))) => {
                log::info!("Exited normally, waiting for next config...");
                cfg_fut.await?
            }
            Ok(Either::Right((cfg, _))) => cfg,
            Err(Either::Left((e, cfg_fut))) => {
                log::info!("Error: {:?}, waiting for next config...", e);
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

pub async fn run_once(ctl: &controller::Controller, config: config::Config) -> Result<()> {
    let wrap_net = {
        let c = ctl.clone();
        move |net: Net| c.get_net(net)
    };
    let registry = load_plugins(config.plugin_path)?;
    log::debug!("registry: {:?}", registry);

    let net = init_net(&registry, config.net, config.rule)?;
    let servers = init_server(&registry, &net, config.server, wrap_net)?;

    log::info!("proxy {:#?}", net.keys());
    log::info!("server {:#?}", servers);

    let mut tasks: FuturesUnordered<_> = servers
        .into_iter()
        .map(|i| start_server(i.server).boxed())
        .collect();

    while tasks.next().await.is_some() {}

    log::info!("all servers are down, exit.");

    Ok(())
}

impl RabbitDigger {
    pub fn new(config_path: PathBuf) -> Result<RabbitDigger> {
        let config_stream = notify_stream(&config_path, RecursiveMode::Recursive)?;
        Ok(RabbitDigger {
            config_stream,
            config_path,
        })
    }
    pub async fn run(self, controller: &controller::Controller) -> Result<()> {
        let config_path = self.config_path;
        let config_stream = stream::once(async { Ok(()) })
            .chain(
                self.config_stream
                    .try_filter(|e| ready(e.kind.is_modify()))
                    .map_err(Into::<anyhow::Error>::into)
                    .map(|_| Ok(())),
            )
            .and_then(move |_| read_to_string(config_path.clone()).map_err(Into::into))
            .and_then(|s| ready(serde_yaml::from_str(&s).map_err(Into::into)));
        pin_mut!(config_stream);
        run(controller, config_stream).await
    }
}

fn get_local(registry: &Registry) -> Result<Net> {
    let net_item = &registry.get_net("local")?;
    let local = net_item.build(Vec::new(), Value::Null)?;
    Ok(local)
}

async fn start_server(server: Server) -> Result<()> {
    server.start().await?;
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
    config: config::ConfigNet,
    rule_config: config::ConfigRule,
) -> Result<HashMap<String, Net>> {
    let mut net: HashMap<String, Net> = HashMap::new();
    let noop = Arc::new(NotImplementedNet);

    net.insert("noop".to_string(), noop.clone());
    net.insert("local".to_string(), get_local(&registry)?);

    for i in config.into_iter() {
        let name = &i.name;
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

        log::trace!("Loading net: {}", name);
        let proxy = net_item.build(chains, i.rest).context(format!(
            "Failed to build net {:?}. Please check your config.",
            name
        ))?;
        net.insert(i.name, proxy);
    }

    net.insert("rule".to_string(), Rule::new(net.clone(), rule_config)?);

    Ok(net)
}

fn init_server(
    registry: &Registry,
    net: &HashMap<String, Net>,
    config: config::ConfigServer,
    wrapper: impl Fn(Net) -> Net,
) -> Result<Vec<ServerInfo>> {
    let mut servers: Vec<ServerInfo> = Vec::new();

    for i in config {
        let server_item = registry.get_server(&i.server_type)?;
        let listen = net.get(&i.listen).ok_or(anyhow!(
            "Listen Net {} is not loaded. Required by {:?}",
            &i.net,
            &i.name
        ))?;
        let net = net.get(&i.net).ok_or(anyhow!(
            "Net {} is not loaded. Required by {:?}",
            &i.net,
            &i.name
        ))?;
        log::trace!("Loading server: {}", i.name);
        let server = server_item
            .build(listen.clone(), wrapper(net.clone()), i.rest.clone())
            .context(format!(
                "Failed to build server {:?}. Please check your config.",
                i.name
            ))?;
        servers.push(ServerInfo {
            name: i.name,
            server,
            config: i.rest,
            listen: i.listen,
            net: i.net,
        });
    }

    Ok(servers)
}
