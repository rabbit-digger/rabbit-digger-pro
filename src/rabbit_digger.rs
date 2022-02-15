use std::{collections::BTreeMap, fmt, time::Duration};

use crate::{
    config,
    rabbit_digger::running::{RunningNet, RunningServer, RunningServerNet},
    registry::Registry,
    util::{topological_sort, RootType},
};
use anyhow::{anyhow, Context, Result};
use futures::{
    future::{try_select, Either},
    stream::FuturesUnordered,
    Stream, StreamExt, TryStreamExt,
};
use rd_interface::{
    config::EmptyConfig, registry::NetGetter, schemars::schema::RootSchema, Arc, IntoDyn, Net,
    Value,
};
use rd_std::builtin::local::LocalNetConfig;
use serde::{Deserialize, Serialize};
use tokio::{
    pin,
    sync::{mpsc, RwLock},
    task::unconstrained,
    time::{sleep, timeout},
};
use uuid::Uuid;

use self::{
    connection::ConnectionConfig,
    connection_manager::{ConnectionManager, ConnectionState},
    event::Event,
};

mod connection;
mod connection_manager;
mod event;
mod running;

pub type PluginLoader =
    Arc<dyn Fn(&config::Config, &mut Registry) -> Result<()> + Send + Sync + 'static>;

#[allow(dead_code)]
struct Running {
    config: RwLock<config::Config>,
    registry_schema: RegistrySchema,
    nets: BTreeMap<String, Arc<RunningNet>>,
    servers: BTreeMap<String, ServerInfo>,
}

enum State {
    WaitConfig,
    Running(Running),
}

impl State {
    fn running(&self) -> Option<&Running> {
        match self {
            State::Running(running) => Some(running),
            _ => None,
        }
    }
}

struct Inner {
    state: RwLock<State>,
    conn_cfg: ConnectionConfig,
}

#[derive(Clone)]
pub struct RabbitDigger {
    manager: ConnectionManager,
    inner: Arc<Inner>,
    registry: Arc<Registry>,
}

impl fmt::Debug for RabbitDigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RabbitDigger").finish()
    }
}

impl RabbitDigger {
    async fn recv_event(mut rx: mpsc::UnboundedReceiver<Event>, conn_mgr: ConnectionManager) {
        loop {
            let e = match rx.try_recv() {
                Ok(e) => e,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
                Err(mpsc::error::TryRecvError::Empty) => {
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            let mut events = Vec::with_capacity(32);
            events.push(e);
            while let Ok(e) = rx.try_recv() {
                events.push(e);
            }
            conn_mgr.input_events(events.into_iter());
        }
        tracing::warn!("recv_event task exited");
    }
    pub async fn new(registry: Registry) -> Result<RabbitDigger> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let manager = ConnectionManager::new();

        tokio::spawn(unconstrained(Self::recv_event(
            event_receiver,
            manager.clone(),
        )));

        let inner = Inner {
            state: RwLock::new(State::WaitConfig),
            conn_cfg: ConnectionConfig::new(event_sender),
        };

        Ok(RabbitDigger {
            inner: Arc::new(inner),
            registry: Arc::new(registry),
            manager,
        })
    }
    pub async fn stop(&self) -> Result<()> {
        let inner = &self.inner;
        let state = inner.state.read().await;

        match &*state {
            State::Running(Running { servers, .. }) => {
                for i in servers.values() {
                    i.running_server.stop().await?;
                }
            }
            _ => {}
        };
        // release the lock to allow other join tasks to write the state
        drop(state);

        self.join().await?;

        Ok(())
    }
    pub async fn join(&self) -> Result<()> {
        let inner = &self.inner;

        match &*inner.state.read().await {
            State::WaitConfig => return Ok(()),
            State::Running(Running { servers, .. }) => {
                let mut race = FuturesUnordered::new();
                for (name, i) in servers {
                    race.push(async move {
                        i.running_server.join().await;
                        if let Some(result) = i.running_server.take_result().await {
                            (name, result)
                        } else {
                            tracing::warn!("Failed to take result. This shouldn't happend");
                            (name, Ok(()))
                        }
                    });
                }

                while let Some((name, r)) = race.next().await {
                    if let Err(e) = r {
                        tracing::warn!("Server {} stopped with error: {:?}", name, e);
                    }
                }
            }
        };

        let state = &mut *inner.state.write().await;
        *state = State::WaitConfig;

        Ok(())
    }

    // get current config if it's running
    pub async fn config(&self) -> Result<config::Config> {
        let state = self.inner.state.read().await;
        match &*state {
            State::Running(Running { config, .. }) => {
                return Ok(config.read().await.clone());
            }
            _ => {
                return Err(anyhow!("Not running"));
            }
        };
    }

    // get current connection state
    pub async fn connection<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&ConnectionState) -> R,
    {
        self.manager.borrow_state(f)
    }

    // get state
    pub async fn state_str(&self) -> Result<&'static str> {
        let state = self.inner.state.read().await;
        Ok(match &*state {
            State::WaitConfig => "WaitConfig",
            State::Running { .. } => "Running",
        })
    }

    // get registry schema
    pub async fn registry<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Option<&RegistrySchema>) -> R,
    {
        let state = self.inner.state.read().await;
        f(state.running().map(|i| &i.registry_schema))
    }

    // start all server, all server run in background.
    pub async fn start(&self, config: config::Config) -> Result<()> {
        let inner = &self.inner;

        self.stop().await?;

        let state = &mut *inner.state.write().await;
        tracing::debug!("Registry:\n{}", self.registry);

        let root = config
            .server
            .values()
            .flat_map(|i| {
                Result::<_, anyhow::Error>::Ok(
                    self.registry
                        .get_server(&i.server_type)?
                        .resolver
                        .get_dependency(i.opt.clone())?,
                )
            })
            .flatten()
            .collect();
        let nets =
            build_nets(&self.registry, config.net.clone(), root).context("Failed to build net")?;
        let servers = build_server(&config.server)
            .await
            .context("Failed to build server")?;
        tracing::debug!(
            "net and server are built. net count: {}, server count: {}",
            nets.len(),
            servers.len()
        );

        tracing::info!("Server:\n{}", ServerList(&servers));
        for (
            _,
            ServerInfo {
                name,
                running_server,
                config,
            },
        ) in &servers
        {
            let item = self.registry.get_server(&running_server.server_type())?;
            let server = item.build(
                &|key| {
                    nets.get(key).map(|i| {
                        let net = i.as_net();

                        RunningServerNet::new(name.clone(), net.clone(), inner.conn_cfg.clone())
                            .into_dyn()
                    })
                },
                config.clone(),
            )?;
            running_server.start(server, &config).await?;
        }

        *state = State::Running(Running {
            config: RwLock::new(config),
            registry_schema: get_registry_schema(&self.registry),
            nets,
            servers,
        });

        Ok(())
    }

    pub async fn is_running(&self) -> bool {
        matches!(*self.inner.state.read().await, State::Running { .. })
    }

    pub async fn start_stream<S>(self, config_stream: S) -> Result<()>
    where
        S: Stream<Item = Result<config::Config>>,
    {
        futures::pin_mut!(config_stream);

        let mut config = match timeout(Duration::from_secs(10), config_stream.try_next()).await {
            Ok(Ok(Some(cfg))) => cfg,
            Ok(Err(e)) => return Err(e.context("Failed to get first config.")),
            Err(_) | Ok(Ok(None)) => {
                return Err(anyhow!("The config_stream is empty, can not start."))
            }
        };

        loop {
            tracing::info!("rabbit digger is starting...");

            self.start(config).await?;

            let new_config = {
                let join_fut = self.join();
                pin!(join_fut);

                match try_select(join_fut, config_stream.try_next()).await {
                    Ok(Either::Left((_, cfg_fut))) => {
                        tracing::info!("Exited normally, waiting for next config...");
                        cfg_fut.await
                    }
                    Ok(Either::Right((cfg, _))) => Ok(cfg),
                    Err(Either::Left((e, cfg_fut))) => {
                        tracing::error!(
                            "Rabbit digger went to error: {:?}, waiting for next config...",
                            e
                        );
                        cfg_fut.await
                    }
                    Err(Either::Right((e, _))) => Err(e),
                }
            };

            config = match new_config? {
                Some(v) => v,
                None => break,
            };

            self.stop().await?;
        }

        Ok(())
    }

    pub async fn get_net(&self, name: &str) -> Result<Option<Arc<RunningNet>>> {
        let state = self.inner.state.read().await;
        match &*state {
            State::Running(Running { nets, .. }) => Ok(nets.get(name).cloned()),
            _ => Err(anyhow!("Not running")),
        }
    }

    // Update net when running.
    pub async fn update_net<F>(&self, net_name: &str, update: F) -> Result<()>
    where
        F: FnOnce(&mut config::Net),
    {
        let state = self.inner.state.read().await;
        match &*state {
            State::Running(Running { config, nets, .. }) => {
                let mut config = config.write().await;
                if let (Some(cfg), Some(running_net)) =
                    (config.net.get_mut(net_name), nets.get(net_name))
                {
                    let mut new_cfg = cfg.clone();
                    update(&mut new_cfg);

                    let net = build_net(net_name, &new_cfg, &self.registry, &|key| {
                        nets.get(key).map(|i| i.as_net())
                    })?;
                    running_net.update_net(net);

                    *cfg = new_cfg;
                }
                return Ok(());
            }
            _ => {
                return Err(anyhow!("Not running"));
            }
        };
    }

    pub async fn get_config<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Option<&config::Config>) -> R,
    {
        let state = self.inner.state.read().await;
        match state.running() {
            Some(i) => f(Some(&*i.config.read().await)),
            None => f(None),
        }
    }

    // Stop the connection by uuid
    pub async fn stop_connection(&self, uuid: Uuid) -> Result<bool> {
        Ok(self.manager.stop_connection(uuid))
    }
}

#[derive(Clone)]
pub struct ServerInfo {
    name: String,
    running_server: RunningServer,
    config: Value,
}

struct ServerList<'a>(&'a BTreeMap<String, ServerInfo>);

impl fmt::Display for ServerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.config)
    }
}

impl<'a> fmt::Display for ServerList<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in self.0.values() {
            writeln!(f, "\t{}", i)?;
        }
        Ok(())
    }
}

fn build_net(name: &str, i: &config::Net, registry: &Registry, getter: NetGetter) -> Result<Net> {
    let net_item = registry.get_net(&i.net_type)?;

    let net = net_item.build(getter, i.opt.clone()).context(format!(
        "Failed to build net {:?}. Please check your config.",
        name
    ))?;

    Ok(net)
}

fn build_nets(
    registry: &Registry,
    mut all_net: config::ConfigNet,
    root: Vec<String>,
) -> Result<BTreeMap<String, Arc<RunningNet>>> {
    let mut running_map: BTreeMap<String, Arc<RunningNet>> = BTreeMap::new();

    if !all_net.contains_key("noop") {
        all_net.insert(
            "noop".to_string(),
            config::Net::new_opt("noop", EmptyConfig::default())?,
        );
    }
    if !all_net.contains_key("local") {
        all_net.insert(
            "local".to_string(),
            config::Net::new_opt("local", LocalNetConfig::default())?,
        );
    }

    let all_net = topological_sort(RootType::Key(root), all_net.into_iter(), |k, n| {
        registry
            .get_net(&n.net_type)?
            .resolver
            .get_dependency(n.opt.clone())
            .context(format!("Failed to get_dependency for net/server: {}", k))
    })
    .context("Failed to do topological_sort")?
    .ok_or_else(|| anyhow!("There is cyclic dependencies in net",))?;

    for (name, i) in all_net {
        let net = build_net(&name, &i, registry, &|key| {
            running_map.get(key).map(|i| i.as_net())
        })
        .context(format!("Loading net {}", name))?;

        let net = RunningNet::new(name.to_string(), net);

        running_map.insert(name.to_string(), net);
    }

    Ok(running_map)
}

async fn build_server(config: &config::ConfigServer) -> Result<BTreeMap<String, ServerInfo>> {
    let mut servers = BTreeMap::new();
    let config = config.clone();

    for (name, i) in config {
        let name = &name;

        let load_server = async {
            let server = RunningServer::new(name.to_string(), i.server_type);
            servers.insert(
                name.to_string(),
                ServerInfo {
                    name: name.to_string(),
                    running_server: server,
                    config: i.opt,
                },
            );
            Ok(()) as Result<()>
        };

        load_server
            .await
            .context(format!("Loading server {}", name))?;
    }

    Ok(servers)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrySchema {
    net: BTreeMap<String, RootSchema>,
    server: BTreeMap<String, RootSchema>,
}

fn get_registry_schema(registry: &Registry) -> RegistrySchema {
    let mut r = RegistrySchema {
        net: BTreeMap::new(),
        server: BTreeMap::new(),
    };

    for (key, value) in registry.net() {
        r.net.insert(key.clone(), value.resolver.schema().clone());
    }
    for (key, value) in registry.server() {
        r.server
            .insert(key.clone(), value.resolver.schema().clone());
    }

    r
}
