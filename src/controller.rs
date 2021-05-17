mod event;
mod wrapper;

use crate::{config, Registry};

use self::event::{BatchEvent, Event, EventType};
use anyhow::Result;
use futures::FutureExt;
use rd_interface::{
    async_trait, schemars::schema::RootSchema, Address, Context, INet, IntoDyn, Net, TcpListener,
    TcpStream, UdpSocket,
};
use serde_derive::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tokio::{
    sync::mpsc,
    sync::{RwLock, RwLockReadGuard},
    task::spawn,
    time::sleep,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrySchema {
    net: HashMap<String, RootSchema>,
    server: HashMap<String, RootSchema>,
}

pub struct Inner {
    config: Option<config::Config>,
    registry: Option<RegistrySchema>,
    sender: broadcast::Sender<BatchEvent>,
}

#[derive(Debug)]
pub struct TaskInfo {
    pub name: String,
}

#[derive(Clone)]
pub struct Controller {
    inner: Arc<RwLock<Inner>>,
    event_sender: mpsc::UnboundedSender<Event>,
}

pub struct ControllerNet {
    net: Net,
    sender: mpsc::UnboundedSender<Event>,
}

#[async_trait]
impl INet for ControllerNet {
    async fn tcp_connect(
        &self,
        ctx: &mut Context,
        addr: Address,
    ) -> rd_interface::Result<TcpStream> {
        let tcp = self.net.tcp_connect(ctx, addr.clone()).await?;
        let tcp = wrapper::TcpStream::new(tcp, self.sender.clone());
        tcp.send(EventType::NewTcp(addr));
        Ok(tcp.into_dyn())
    }

    // TODO: wrap TcpListener
    async fn tcp_bind(
        &self,
        ctx: &mut Context,
        addr: Address,
    ) -> rd_interface::Result<TcpListener> {
        self.net.tcp_bind(ctx, addr).await
    }

    // TODO: wrap UdpSocket
    async fn udp_bind(&self, ctx: &mut Context, addr: Address) -> rd_interface::Result<UdpSocket> {
        self.net.udp_bind(ctx, addr).await
    }
}

async fn process(mut rx: mpsc::UnboundedReceiver<Event>, sender: broadcast::Sender<BatchEvent>) {
    loop {
        let e = match rx.recv().now_or_never() {
            Some(Some(e)) => e,
            Some(None) => break,
            None => {
                sleep(Duration::from_millis(100)).await;
                continue;
            }
        };

        let mut events = BatchEvent::with_capacity(16);
        events.push(Arc::new(e));
        while let Some(Some(e)) = rx.recv().now_or_never() {
            events.push(Arc::new(e));
        }

        // Failed only when no receiver
        sender.send(events).ok();
    }
}

impl Controller {
    pub fn new() -> Controller {
        let (sender, _) = broadcast::channel(16);
        let inner = Arc::new(RwLock::new(Inner {
            config: None,
            registry: None,
            sender: sender.clone(),
        }));
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        spawn(process(event_receiver, sender));
        Controller {
            inner,
            event_sender,
        }
    }
    pub fn get_net(&self, net: Net) -> Net {
        ControllerNet {
            net,
            sender: self.event_sender.clone(),
        }
        .into_dyn()
    }
    pub(crate) async fn update_registry(&self, registry: &Registry) -> Result<()> {
        let mut inner = self.inner.write().await;
        if inner.registry.is_some() {
            anyhow::bail!("this controller already has a registry")
        }
        let mut r = RegistrySchema {
            net: HashMap::new(),
            server: HashMap::new(),
        };

        for (key, value) in &registry.net {
            r.net.insert(key.clone(), value.resolver.schema().clone());
        }
        for (key, value) in &registry.server {
            r.server
                .insert(key.clone(), value.resolver.schema().clone());
        }

        inner.registry = Some(r);
        Ok(())
    }
    pub(crate) async fn remove_registry(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.registry = None;
        Ok(())
    }
    pub(crate) async fn update_config(&self, config: config::Config) -> Result<()> {
        let mut inner = self.inner.write().await;
        if inner.config.is_some() {
            anyhow::bail!("this controller already has a config")
        }
        inner.config = Some(config);
        Ok(())
    }
    pub(crate) async fn remove_config(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.config = None;
        Ok(())
    }
    pub async fn lock<'a>(&'a self) -> RwLockReadGuard<'a, Inner> {
        self.inner.read().await
    }
    pub async fn get_subscriber(&self) -> broadcast::Receiver<BatchEvent> {
        self.inner.read().await.sender.subscribe()
    }
}

impl Inner {
    pub fn config(&self) -> Option<&config::Config> {
        self.config.as_ref()
    }
    pub fn registry(&self) -> Option<&RegistrySchema> {
        self.registry.as_ref()
    }
}
