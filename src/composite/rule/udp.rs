use std::{net::SocketAddr, time::Duration};

use super::rule::Rule;
use async_std::{
    channel::{bounded, Receiver, Sender},
    sync::RwLock,
    task::spawn,
};
use futures::{
    channel::oneshot,
    future::{select, Either},
};
use lru_time_cache::LruCache;
use rd_interface::{
    async_trait, constant::UDP_BUFFER_SIZE, Address, Context, IUdpSocket, Result, UdpSocket,
    NOT_IMPLEMENTED,
};

type UdpPacket = (Vec<u8>, SocketAddr);
type NatTable = RwLock<LruCache<Address, DroppableUdpSocket>>;

pub struct UdpRuleSocket {
    rule: Rule,
    context: Context,
    nat: NatTable,
    tx: Sender<UdpPacket>,
    rx: Receiver<UdpPacket>,
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
    pub fn new(rule: Rule, context: Context) -> UdpRuleSocket {
        let (tx, rx) = bounded::<UdpPacket>(100);
        let nat = RwLock::new(LruCache::with_expiry_duration_and_capacity(
            Duration::from_secs(10 * 60),
            100,
        ));

        UdpRuleSocket {
            rule,
            context,
            nat,
            tx,
            rx,
        }
    }
}

#[async_trait]
impl IUdpSocket for UdpRuleSocket {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        let (data, addr) = self
            .rx
            .recv()
            .await
            .map_err(|_| rd_interface::Error::Other("Failed to receive UDP".into()))?;

        let to_copy = data.len().min(buf.len());
        buf[..to_copy].copy_from_slice(&data[..to_copy]);

        Ok((to_copy, addr))
    }

    async fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize> {
        let mut ctx = self.context.clone();
        let nat_has = self.nat.read().await.contains_key(&addr);

        if nat_has {
            if let Some(udp) = self.nat.write().await.get(&addr) {
                return udp.0.send_to(buf, addr).await;
            }
        }

        let udp = self
            .rule
            .get_rule(&mut ctx, &addr)
            .await?
            .target
            .udp_bind(&mut ctx, addr.clone())
            .await?;
        let size = udp.send_to(buf, addr.clone()).await?;

        self.nat
            .write()
            .await
            .insert(addr.clone(), DroppableUdpSocket::new(udp, self.tx.clone()));

        Ok(size)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}
