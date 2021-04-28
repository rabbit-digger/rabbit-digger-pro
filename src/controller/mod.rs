mod event;
mod wrapper;

use self::event::{Event, EventType};
use anyhow::Result;
use async_std::{
    channel,
    task::{sleep, spawn},
};
use rd_interface::{
    async_trait, Address, Context, INet, IntoDyn, Net, TcpListener, TcpStream, UdpSocket,
};
use std::{sync::Arc, time::Duration};

struct Inner {}

#[derive(Debug)]
pub struct TaskInfo {
    pub name: String,
}

#[derive(Clone)]
pub struct Controller {
    inner: Arc<Inner>,
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

    // TODO: wrap TcpListener
    async fn udp_bind(&self, ctx: &mut Context, addr: Address) -> rd_interface::Result<UdpSocket> {
        self.net.udp_bind(ctx, addr).await
    }
}

async fn process(rx: channel::Receiver<Event>) {
    loop {
        let e = match rx.try_recv() {
            Ok(e) => e,
            Err(channel::TryRecvError::Empty) => {
                sleep(Duration::from_millis(100)).await;
                continue;
            }
            Err(channel::TryRecvError::Closed) => break,
        };

        log::trace!("Event {:?}", e);
    }
}

impl Controller {
    pub fn new() -> Controller {
        let inner = Arc::new(Inner {});
        let (sender, rx) = channel::unbounded();
        spawn(process(rx));
        Controller { inner, sender }
    }
    pub fn get_net(&self, net: Net) -> Net {
        ControllerNet {
            net,
            sender: self.sender.clone(),
        }
        .into_dyn()
    }
    pub async fn start(&self) -> Result<()> {
        spawn(serve(self.inner.clone()));
        Ok(())
    }
}

async fn serve(inner: Arc<Inner>) {
    //
}
