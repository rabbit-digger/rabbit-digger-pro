use std::{net::SocketAddr, time::Duration};

use super::rule_net::Rule;
use futures::{
    channel::oneshot,
    future::{select, Either},
};
use lru_time_cache::LruCache;
use rd_interface::{
    async_trait, constant::UDP_BUFFER_SIZE, Address, Context, IUdpSocket, Result, UdpSocket,
    NOT_IMPLEMENTED,
};
use tokio::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
    task::spawn,
};

type UdpPacket = (Vec<u8>, SocketAddr);
type NatTable = Mutex<LruCache<String, DroppableUdpSocket>>;

pub struct UdpRuleSocket {
    rule: Rule,
    context: Context,
    nat: NatTable,
    cache: Mutex<LruCache<Address, String>>,
    tx: Sender<UdpPacket>,
    rx: Mutex<Receiver<UdpPacket>>,
    bind_addr: Address,
}

struct DroppableUdpSocket(UdpSocket, Option<oneshot::Sender<()>>);

impl DroppableUdpSocket {
    fn new(udp: UdpSocket, tx: Sender<UdpPacket>) -> DroppableUdpSocket {
        let (stop_sender, mut stop) = oneshot::channel::<()>();
        let u = udp.clone();
        spawn(async move {
            let mut buf = [0u8; UDP_BUFFER_SIZE];
            loop {
                let (size, addr) = match select(u.recv_from(&mut buf), &mut stop).await {
                    Either::Left((r, _)) => r?,
                    Either::Right(_) => break,
                };
                tx.send((buf[0..size].to_vec(), addr)).await?;
            }
            anyhow::Result::<()>::Ok(())
        });
        DroppableUdpSocket(udp, Some(stop_sender))
    }
}

impl Drop for DroppableUdpSocket {
    fn drop(&mut self) {
        if let Some(v) = self.1.take() {
            v.send(()).ok();
        }
    }
}

impl UdpRuleSocket {
    pub fn new(rule: Rule, context: Context, bind_addr: Address) -> UdpRuleSocket {
        let (tx, rx) = channel::<UdpPacket>(100);
        let nat = Mutex::new(LruCache::with_expiry_duration_and_capacity(
            Duration::from_secs(60),
            100,
        ));

        UdpRuleSocket {
            rule,
            context,
            nat,
            cache: Mutex::new(LruCache::with_expiry_duration_and_capacity(
                Duration::from_secs(10 * 60),
                100,
            )),
            tx,
            rx: Mutex::new(rx),
            bind_addr,
        }
    }
    async fn get_net_name<'a>(&'a self, ctx: &Context, addr: &Address) -> Result<String> {
        let mut c = self.cache.lock().await;
        if let Some(v) = c.get(addr) {
            Ok(v.to_string())
        } else {
            let out_net = self.rule.get_rule(ctx, addr).await?.target_name.to_string();
            c.insert(addr.clone(), out_net.clone());
            Ok(out_net)
        }
    }
}

#[async_trait]
impl IUdpSocket for UdpRuleSocket {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        let (data, addr) = self
            .rx
            .lock()
            .await
            .recv()
            .await
            .ok_or(rd_interface::Error::Other("Failed to receive UDP".into()))?;

        let to_copy = data.len().min(buf.len());
        buf[..to_copy].copy_from_slice(&data[..to_copy]);

        Ok((to_copy, addr))
    }

    async fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize> {
        let out_net = self.get_net_name(&self.context, &addr).await?;

        if let Some(udp) = self.nat.lock().await.get(&out_net) {
            return udp.0.send_to(buf, addr).await;
        }

        let udp = self
            .rule
            .get_rule(&self.context, &addr)
            .await?
            .target
            .udp_bind(&mut self.context.clone(), self.bind_addr.clone())
            .await?;

        self.nat.lock().await.insert(
            out_net,
            DroppableUdpSocket::new(udp.clone(), self.tx.clone()),
        );

        Ok(udp.send_to(buf, addr.clone()).await?)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}
