use std::{net::SocketAddr, time::Duration};

use super::rule_net::Rule;
use lru_time_cache::LruCache;
use rd_interface::{
    async_trait, constant::UDP_BUFFER_SIZE, error::map_other, Address, Context, IUdpSocket, Net,
    Result, NOT_IMPLEMENTED,
};
use tokio::{
    select,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver as Receiver, UnboundedSender as Sender},
        Mutex,
    },
    task::spawn,
    time::timeout,
};

type UdpPacket = (Vec<u8>, SocketAddr);
type NatTable = parking_lot::Mutex<LruCache<String, UdpTunnel>>;

pub struct UdpRuleSocket {
    rule: Rule,
    context: Context,
    nat: NatTable,
    cache: Mutex<LruCache<Address, (Net, String)>>,
    tx: Sender<UdpPacket>,
    rx: Mutex<Receiver<UdpPacket>>,
    bind_addr: Address,
}

struct UdpTunnel(Sender<(Vec<u8>, Address)>);

impl UdpTunnel {
    fn new(
        net: Net,
        mut context: Context,
        bind_addr: Address,
        send_back: Sender<UdpPacket>,
    ) -> UdpTunnel {
        let (tx, mut rx) = unbounded_channel::<(Vec<u8>, Address)>();
        spawn(async move {
            let udp = timeout(
                Duration::from_secs(5),
                net.udp_bind(&mut context, &bind_addr),
            )
            .await
            .map_err(map_other)??;

            let send = async {
                while let Some((packet, addr)) = rx.recv().await {
                    if let Err(e) = udp.send_to(&packet, addr).await {
                        tracing::error!("drop packet: {:?}", e);
                    }
                }

                anyhow::Result::<()>::Ok(())
            };
            let recv = async {
                let mut buf = [0u8; UDP_BUFFER_SIZE];
                loop {
                    let (size, addr) = udp.recv_from(&mut buf).await?;

                    if send_back.send((buf[..size].to_vec(), addr)).is_err() {
                        break;
                    }
                }
                tracing::trace!("send_raw return error");
                anyhow::Result::<()>::Ok(())
            };

            select! {
                r = send => r?,
                r = recv => r?,
            }

            anyhow::Result::<()>::Ok(())
        });
        UdpTunnel(tx)
    }
    fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize> {
        match self.0.send((buf.to_vec(), addr)) {
            Err(_) => Err(rd_interface::Error::Other("Other side closed".into())),
            Ok(_) => Ok(buf.len()),
        }
    }
}

impl UdpRuleSocket {
    pub fn new(rule: Rule, context: Context, bind_addr: Address) -> UdpRuleSocket {
        let (tx, rx) = unbounded_channel::<UdpPacket>();
        let nat: NatTable = parking_lot::Mutex::new(LruCache::with_expiry_duration_and_capacity(
            Duration::from_secs(30),
            128,
        ));

        UdpRuleSocket {
            rule,
            context,
            nat,
            cache: Mutex::new(LruCache::with_expiry_duration_and_capacity(
                Duration::from_secs(30),
                128,
            )),
            tx,
            rx: Mutex::new(rx),
            bind_addr,
        }
    }
    async fn get_net_name(&self, ctx: &Context, addr: &Address) -> Result<(Net, String)> {
        let mut c = self.cache.lock().await;
        if let Some((net, name)) = c.get(addr) {
            Ok((net.clone(), name.clone()))
        } else {
            let rule_item = self.rule.get_rule(ctx, addr).await?;
            let out_net = (rule_item.target.clone(), rule_item.target_name.to_string());
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
            .ok_or_else(|| rd_interface::Error::Other("Failed to receive UDP".into()))?;

        let to_copy = data.len().min(buf.len());
        buf[..to_copy].copy_from_slice(&data[..to_copy]);

        Ok((to_copy, addr))
    }

    async fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize> {
        let (net, out_net) = self.get_net_name(&self.context, &addr).await?;
        let mut nat = self.nat.lock();

        let udp = nat.entry(out_net).or_insert_with(|| {
            UdpTunnel::new(
                net,
                self.context.clone(),
                self.bind_addr.clone(),
                self.tx.clone(),
            )
        });

        udp.send_to(buf, addr)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}
