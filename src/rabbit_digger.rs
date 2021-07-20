use std::{collections::BTreeMap, fmt, time::Duration};

use crate::{
    builtin::load_builtin,
    config,
    rabbit_digger::running::{RunningNet, RunningServer, RunningServerNet},
    registry::Registry,
    util::topological_sort,
};
use anyhow::{anyhow, Context, Result};
use futures::{
    future::{try_select, Either},
    FutureExt, Stream, TryStreamExt,
};
use rd_interface::{config::EmptyConfig, Arc, IntoDyn, Net, Value};
use rd_std::builtin::local::LocalNetConfig;
use tokio::{
    pin,
    sync::{broadcast, mpsc, RwLock},
    time::{sleep, timeout},
};

use self::{
    connection::ConnectionConfig,
    event::{BatchEvent, Event},
};

mod connection;
mod event;
mod running;

pub type PluginLoader =
    Arc<dyn Fn(&config::Config, &mut Registry) -> Result<()> + Send + Sync + 'static>;

#[derive(Clone)]
pub struct RabbitDiggerBuilder {
    plugin_loader: PluginLoader,
}

#[allow(dead_code)]
enum State {
    WaitConfig,
    Running {
        config: config::Config,
        registry: Registry,
        nets: BTreeMap<String, RunningNet>,
        servers: BTreeMap<String, ServerInfo>,
    },
}

struct Inner {
    state: RwLock<State>,
    conn_cfg: ConnectionConfig,
}

#[derive(Clone)]
pub struct RabbitDigger {
    inner: Arc<Inner>,
    plugin_loader: PluginLoader,
}

impl fmt::Debug for RabbitDigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RabbitDigger").finish()
    }
}

impl RabbitDigger {
    async fn recv_event(
        mut rx: mpsc::UnboundedReceiver<Event>,
        sender: broadcast::Sender<BatchEvent>,
    ) {
        loop {
            let e = match rx.recv().now_or_never() {
                Some(Some(e)) => e,
                Some(None) => break,
                None => {
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };

            let mut events = Vec::with_capacity(16);
            events.push(e);
            while let Some(Some(e)) = rx.recv().now_or_never() {
                events.push(e);
            }

            // Failed only when no receiver
            sender.send(Arc::new(events)).ok();
        }
        tracing::trace!("recv_event task exited");
    }
    async fn new(plugin_loader: &PluginLoader) -> Result<RabbitDigger> {
        let (sender, _) = broadcast::channel(16);
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        tokio::spawn(Self::recv_event(event_receiver, sender));

        let inner = Inner {
            state: RwLock::new(State::WaitConfig),
            conn_cfg: ConnectionConfig::new(event_sender),
        };

        Ok(RabbitDigger {
            inner: Arc::new(inner),
            plugin_loader: plugin_loader.clone(),
        })
    }
    async fn stop_inner(&self, state: &mut State) -> Result<()> {
        match state {
            State::Running { servers, .. } => {
                for i in servers.values() {
                    i.server.stop().await?;
                }
            }
            _ => {}
        };

        *state = State::WaitConfig;

        Ok(())
    }
    pub async fn stop(&self) -> Result<()> {
        let inner = &self.inner;

        match &*inner.state.read().await {
            State::WaitConfig => return Ok(()),
            State::Running { .. } => {}
        };

        let state = &mut *inner.state.write().await;

        self.stop_inner(state).await?;

        Ok(())
    }
    pub async fn join(&self) -> Result<()> {
        let inner = &self.inner;

        match &*inner.state.read().await {
            State::WaitConfig => return Ok(()),
            State::Running { servers, .. } => {
                for i in servers.values() {
                    i.server.join().await?;
                }
            }
        };

        let state = &mut *inner.state.write().await;
        *state = State::WaitConfig;

        Ok(())
    }

    // start all server, all server run in background.
    pub async fn start(&self, config: config::Config) -> Result<()> {
        let inner = &self.inner;

        let state = &mut *inner.state.write().await;

        self.stop_inner(state).await?;

        let mut registry = Registry::new();

        load_builtin(&mut registry).context("Failed to load builtin")?;
        (self.plugin_loader)(&config, &mut registry).context("Failed to load plugin")?;
        tracing::debug!("Registry:\n{}", registry);

        let nets = build_net(&registry, config.net.clone()).context("Failed to build net")?;
        let servers = build_server(&nets, &config.server, &inner.conn_cfg)
            .await
            .context("Failed to build server")?;
        tracing::debug!(
            "net and server are built. net count: {}, server count: {}",
            nets.len(),
            servers.len()
        );

        tracing::info!("Server:\n{}", ServerList(&servers));
        for (_, server) in &servers {
            server.server.start(&registry, &server.config).await?;
        }

        *state = State::Running {
            config,
            registry,
            nets,
            servers,
        };

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

        let mut config = match timeout(Duration::from_secs(1), config_stream.try_next()).await {
            Ok(Ok(Some(cfg))) => cfg,
            Ok(Err(e)) => return Err(e.context("Failed to get first config.")),
            Err(_) | Ok(Ok(None)) => {
                return Err(anyhow!("The config_stream is empty, can not start."))
            }
        };

        loop {
            tracing::info!("rabbit digger is starting...");

            self.start(config).await?;

            let join_fut = self.join();
            pin!(join_fut);

            let new_config = match try_select(join_fut, config_stream.try_next()).await {
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
            };

            config = match new_config? {
                Some(v) => v,
                None => break,
            };

            self.stop().await?;
        }

        Ok(())
    }
}

impl Default for RabbitDiggerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RabbitDiggerBuilder {
    pub fn new() -> RabbitDiggerBuilder {
        RabbitDiggerBuilder {
            plugin_loader: Arc::new(|_, _| Ok(())),
        }
    }
    pub fn plugin_loader<PL>(mut self, plugin_loader: PL) -> Self
    where
        PL: Fn(&config::Config, &mut Registry) -> Result<()> + Send + Sync + 'static,
    {
        self.plugin_loader = Arc::new(plugin_loader);
        self
    }
    pub async fn build(&self) -> Result<RabbitDigger> {
        let rd = RabbitDigger::new(&self.plugin_loader).await?;
        Ok(rd)
    }
}

#[derive(Clone)]
pub struct ServerInfo {
    name: String,
    listen: String,
    net: String,
    server: RunningServer,
    config: Value,
}

struct ServerList<'a>(&'a BTreeMap<String, ServerInfo>);

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
        for i in self.0.values() {
            writeln!(f, "\t{}", i)?;
        }
        Ok(())
    }
}

fn build_net(
    registry: &Registry,
    mut all_net: BTreeMap<String, config::Net>,
) -> Result<BTreeMap<String, RunningNet>> {
    let mut net_map: BTreeMap<String, Net> = BTreeMap::new();
    let mut running_map: BTreeMap<String, RunningNet> = BTreeMap::new();

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

    let all_net = topological_sort(all_net, |k, n| {
        registry
            .get_net(&n.net_type)?
            .resolver
            .get_dependency(n.opt.clone())
            .context(format!("Failed to get_dependency for net/server: {}", k))
    })
    .context("Failed to do topological_sort")?
    .ok_or_else(|| anyhow!("There is cyclic dependencies in net",))?;

    for (name, i) in all_net {
        let load_net = || -> Result<()> {
            let net_item = registry.get_net(&i.net_type)?;

            let net = net_item.build(&net_map, i.opt.clone()).context(format!(
                "Failed to build net {:?}. Please check your config.",
                name
            ))?;
            let net = RunningNet::new(name.to_string(), i.opt, net);
            net_map.insert(name.to_string(), net.clone().into_dyn());
            running_map.insert(name.to_string(), net);
            Ok(())
        };
        load_net().context(format!("Loading net {}", name))?;
    }

    Ok(running_map)
}

async fn build_server(
    net: &BTreeMap<String, RunningNet>,
    config: &config::ConfigServer,
    conn_cfg: &ConnectionConfig,
) -> Result<BTreeMap<String, ServerInfo>> {
    let mut servers = BTreeMap::new();
    let config = config.clone();

    for (name, i) in config {
        let name = &name;

        let load_server = async {
            let listen = net
                .get(&i.listen)
                .ok_or_else(|| {
                    anyhow!(
                        "Listen Net {} is not loaded. Required by {:?}",
                        &i.net,
                        &name
                    )
                })?
                .net()
                .await;
            let net = RunningServerNet::new(
                net.get(&i.net)
                    .ok_or_else(|| {
                        anyhow!("Net {} is not loaded. Required by {:?}", &i.net, &name)
                    })?
                    .net()
                    .await,
                conn_cfg.clone(),
            )
            .into_dyn();

            let server = RunningServer::new(name.to_string(), i.server_type, net, listen);
            servers.insert(
                name.to_string(),
                ServerInfo {
                    name: name.to_string(),
                    server,
                    config: i.opt,
                    listen: i.listen,
                    net: i.net,
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
