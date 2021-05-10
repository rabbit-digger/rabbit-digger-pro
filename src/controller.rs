mod event;
mod wrapper;

use crate::config;

use self::event::{Event, EventType};
use async_std::{
    channel,
    sync::{RwLock, RwLockReadGuard},
    task::{sleep, spawn},
};
use futures::FutureExt;
use rd_interface::{
    async_trait, Address, Context, INet, IntoDyn, Net, TcpListener, TcpStream, UdpSocket,
};
use std::{collections::VecDeque, sync::Arc, time::Duration};

pub(crate) struct Inner {
    config: Option<config::Config>,
    events: VecDeque<Event>,
}

#[derive(Debug)]
pub struct TaskInfo {
    pub name: String,
}

#[derive(Clone)]
pub struct Controller {
    inner: Arc<RwLock<Inner>>,
    sender: channel::Sender<Event>,
}

pub struct ControllerNet {
    net: Net,
    sender: channel::Sender<Event>,
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

async fn process(rx: channel::Receiver<Event>, inner: Arc<RwLock<Inner>>) {
    loop {
        let e = match rx.try_recv() {
            Ok(e) => e,
            Err(channel::TryRecvError::Empty) => {
                sleep(Duration::from_millis(100)).await;
                continue;
            }
            Err(channel::TryRecvError::Closed) => break,
        };

        let mut inner = inner.write().await;
        inner.events.push_back(e);
        while let Some(Ok(e)) = rx.recv().now_or_never() {
            inner.events.push_back(e);
        }
    }
}

impl Controller {
    pub fn new() -> Controller {
        let inner = Arc::new(RwLock::new(Inner {
            config: None,
            events: VecDeque::new(),
        }));
        let (sender, rx) = channel::unbounded();
        spawn(process(rx, inner.clone()));
        Controller { inner, sender }
    }
    pub fn get_net(&self, net: Net) -> Net {
        ControllerNet {
            net,
            sender: self.sender.clone(),
        }
        .into_dyn()
    }
    pub async fn update_config(&self, config: config::Config) {
        self.inner.write().await.config = Some(config);
    }
    pub(crate) async fn inner<'a>(&'a self) -> RwLockReadGuard<'a, Inner> {
        self.inner.read().await
    }
}

impl Inner {
    pub fn config(&self) -> Option<&config::Config> {
        self.config.as_ref()
    }
}
