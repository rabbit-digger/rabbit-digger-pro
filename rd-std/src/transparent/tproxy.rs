// UDP: https://github.com/shadowsocks/shadowsocks-rust/blob/0433b3ec09bcaa26f7460a50287b56c67b687a34/crates/shadowsocks-service/src/local/redir/udprelay/sys/unix/linux.rs#L56

use std::{net::SocketAddr, sync::Arc, time::Duration};

use super::socket::{create_tcp_listener, TransparentUdp};
use crate::{
    builtin::local::CompatTcp,
    util::{connect_tcp, is_reserved},
};
use lru_time_cache::LruCache;
use rd_interface::{
    async_trait,
    constant::UDP_BUFFER_SIZE,
    error::map_other,
    registry::ServerFactory,
    schemars::{self, JsonSchema},
    Address, Context, Error, IServer, IntoAddress, IntoDyn, Net, Result,
};
use serde::Deserialize;
use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::{
        mpsc::{unbounded_channel, UnboundedSender as Sender},
        Mutex,
    },
    task::JoinHandle,
    time::timeout,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TProxyServerConfig {
    bind: Address,
    mark: Option<u32>,
}

pub struct TProxyServer {
    cfg: TProxyServerConfig,
    net: Net,
}

#[async_trait]
impl IServer for TProxyServer {
    async fn start(&self) -> Result<()> {
        let tcp_listener = create_tcp_listener(self.cfg.bind.to_socket_addr()?).await?;
        let udp_listener = TransparentUdp::listen(self.cfg.bind.to_socket_addr()?).await?;

        select! {
            r = self.serve_listener(tcp_listener) => r,
            r = self.serve_udp(udp_listener) => r,
        }
    }
}

impl TProxyServer {
    pub fn new(cfg: TProxyServerConfig, net: Net) -> Self {
        TProxyServer { cfg, net }
    }

    async fn serve_udp(&self, listener: TransparentUdp) -> Result<()> {
        let mut buf = [0u8; 4096];
        let net = self.net.clone();
        let mut nat = LruCache::<SocketAddr, UdpTunnel>::with_expiry_duration_and_capacity(
            Duration::from_secs(30),
            128,
        );
        let cache = Arc::new(TransparentUdpCache::new(self.cfg.mark));

        loop {
            let (size, src, dst) = match listener.recv(&mut buf).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("UDP recv error: {:?}", e);
                    continue;
                }
            };

            let payload = &buf[..size];

            let udp = nat
                .entry(src)
                .or_insert_with(|| UdpTunnel::new(net.clone(), src, cache.clone()));

            if let Err(e) = udp.send_to(payload, dst).await {
                tracing::error!("Udp send_to {:?}", e);
                nat.remove(&src);
            }
        }
    }

    async fn serve_listener(&self, listener: TcpListener) -> Result<()> {
        loop {
            let (socket, addr) = listener.accept().await?;

            let net = self.net.clone();
            let _ = tokio::spawn(async move {
                if let Err(e) = Self::serve_connection(net, socket, addr).await {
                    tracing::error!("Error when serve_connection: {:?}", e);
                }
            });
        }
    }

    async fn serve_connection(net: Net, socket: TcpStream, addr: SocketAddr) -> Result<()> {
        let target = socket.local_addr()?;

        let ctx = &mut Context::from_socketaddr(addr);
        let target_tcp = net.tcp_connect(ctx, &target.into_address()?).await?;
        let socket = CompatTcp(socket).into_dyn();

        connect_tcp(ctx, socket, target_tcp).await?;

        Ok(())
    }
}

impl ServerFactory for TProxyServer {
    const NAME: &'static str = "tproxy";
    type Config = TProxyServerConfig;
    type Server = Self;

    fn new(_: Net, net: Net, config: Self::Config) -> Result<Self> {
        Ok(TProxyServer::new(config, net))
    }
}

struct TransparentUdpCache {
    mark: Option<u32>,
    cache: Mutex<LruCache<SocketAddr, TransparentUdp>>,
}

impl TransparentUdpCache {
    fn new(mark: Option<u32>) -> Self {
        TransparentUdpCache {
            mark,
            cache: Mutex::new(LruCache::with_expiry_duration_and_capacity(
                Duration::from_secs(30),
                128,
            )),
        }
    }
    async fn send_to(
        &self,
        from_addr: SocketAddr,
        to_addr: SocketAddr,
        buf: &[u8],
    ) -> Result<usize> {
        // Don't cache reserved address
        if is_reserved(from_addr.ip()) {
            let udp = TransparentUdp::bind_any(from_addr, self.mark).await?;
            Ok(udp.send_to(buf, to_addr).await?)
        } else {
            let mut cache = self.cache.lock().await;
            let back_udp = match cache.get(&from_addr) {
                Some(udp) => udp,
                None => {
                    let udp = TransparentUdp::bind_any(from_addr, self.mark).await?;
                    cache.insert(from_addr, udp);
                    cache
                        .get(&from_addr)
                        .expect("impossible: failed to get by from_addr")
                }
            };

            Ok(back_udp.send_to(buf, to_addr).await?)
        }
    }
}

struct UdpTunnel {
    tx: Sender<(SocketAddr, Vec<u8>)>,
    handle: Mutex<Option<JoinHandle<Result<()>>>>,
}

impl UdpTunnel {
    fn new(net: Net, src: SocketAddr, cache: Arc<TransparentUdpCache>) -> UdpTunnel {
        let (tx, mut rx) = unbounded_channel::<(SocketAddr, Vec<u8>)>();
        let handle = tokio::spawn(async move {
            let udp = timeout(
                Duration::from_secs(5),
                net.udp_bind(
                    &mut Context::from_socketaddr(src),
                    &Address::any_addr_port(&src),
                ),
            )
            .await
            .map_err(map_other)??;

            let send = async {
                while let Some((addr, packet)) = rx.recv().await {
                    udp.send_to(&packet, addr.into()).await?;
                }
                Ok(()) as Result<()>
            };
            let recv = async {
                let mut buf = [0u8; UDP_BUFFER_SIZE];
                loop {
                    let (size, addr) = udp.recv_from(&mut buf).await?;

                    cache.send_to(addr, src, &buf[..size]).await?;
                }
            };

            let r: Result<()> = select! {
                r = send => r,
                r = recv => r,
            };

            if let Err(e) = &r {
                tracing::error!("tproxy error {:?} src {}", e, src);
            }

            Ok(()) as Result<()>
        });
        UdpTunnel {
            tx,
            handle: Mutex::new(handle.into()),
        }
    }
    /// return false if the send queue is full
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<()> {
        match self.tx.send((addr, buf.to_vec())) {
            Ok(_) => Ok(()),
            Err(_) => {
                let mut handle = self.handle.lock().await;
                if let Some(handle) = handle.take() {
                    let r = handle.await;
                    tracing::error!("Other side closed: {:?}", r);
                }
                Err(Error::Other("Other side closed".into()))
            }
        }
    }
}
