// UDP: https://github.com/shadowsocks/shadowsocks-rust/blob/0433b3ec09bcaa26f7460a50287b56c67b687a34/crates/shadowsocks-service/src/local/redir/udprelay/sys/unix/linux.rs#L56

use std::{io, net::SocketAddr, pin::Pin, task, time::Duration};

use super::socket::{create_tcp_listener, TransparentUdp};
use crate::{
    builtin::local::CompatTcp,
    util::{
        connect_tcp,
        forward_udp::{forward_udp, RawUdpSource, UdpPacket},
        is_reserved, LruCache,
    },
};
use futures::{ready, Sink, Stream};
use rd_derive::rd_config;
use rd_interface::{
    async_trait, constant::UDP_BUFFER_SIZE, error::ErrorContext, registry::ServerBuilder, schemars,
    Address, Bytes, Context, IServer, IntoAddress, IntoDyn, Net, Result,
};
use tokio::{
    net::{TcpListener, TcpStream},
    select,
};

#[rd_config]
#[derive(Debug)]
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
        let udp_listener = TransparentUdp::listen(self.cfg.bind.to_socket_addr()?)?;

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
        let source = UdpSource::new(listener, self.cfg.mark);

        forward_udp(source, self.net.clone(), None)
            .await
            .context("forward udp")?;

        Ok(())
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

impl ServerBuilder for TProxyServer {
    const NAME: &'static str = "tproxy";
    type Config = TProxyServerConfig;
    type Server = Self;

    fn build(_: Net, net: Net, config: Self::Config) -> Result<Self> {
        Ok(TProxyServer::new(config, net))
    }
}

#[derive(Debug)]
enum SendState {
    Idle,
    Sending(UdpPacket),
}

struct UdpSource {
    recv_buf: Box<[u8]>,
    tudp: TransparentUdp,

    mark: Option<u32>,
    cache: LruCache<SocketAddr, TransparentUdp>,
    send_state: SendState,
}

impl UdpSource {
    fn new(tudp: TransparentUdp, mark: Option<u32>) -> Self {
        UdpSource {
            recv_buf: Box::new([0; UDP_BUFFER_SIZE]),
            tudp,
            mark,
            cache: LruCache::with_expiry_duration_and_capacity(Duration::from_secs(30), 128),
            send_state: SendState::Idle,
        }
    }
    fn poll_send_to(&mut self, cx: &mut task::Context<'_>) -> task::Poll<io::Result<()>> {
        let UdpSource {
            cache, send_state, ..
        } = self;

        match send_state {
            SendState::Idle => {}
            SendState::Sending(UdpPacket { from, to, data }) => {
                let udp = match cache.get(&from) {
                    Some(udp) => udp,
                    None => {
                        let result = TransparentUdp::bind_any(*from, self.mark);
                        let udp = match result {
                            Ok(udp) => udp,
                            Err(e) => {
                                tracing::error!(
                                    "Failed to bind any addr: {}. Reason: {:?}",
                                    from,
                                    e
                                );

                                *send_state = SendState::Idle;

                                return Ok(()).into();
                            }
                        };
                        cache.insert(*from, udp);
                        cache
                            .get(&from)
                            .expect("impossible: failed to get by from_addr")
                    }
                };

                ready!(udp.poll_send_to(cx, data, *to))?;

                // Don't cache reserved address
                if is_reserved(from.ip()) {
                    cache.remove(&from);
                }
            }
        }

        *send_state = SendState::Idle;

        Ok(()).into()
    }
}

impl Stream for UdpSource {
    type Item = io::Result<UdpPacket>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let UdpSource {
            recv_buf,
            tudp,
            cache,
            ..
        } = self.get_mut();
        cache.poll_clear_expired(cx);
        let (size, from, to) = ready!(tudp.poll_recv(cx, &mut recv_buf[..]))?;

        Some(Ok(UdpPacket::new(
            Bytes::copy_from_slice(&recv_buf[..size]),
            from,
            to,
        )))
        .into()
    }
}

impl Sink<UdpPacket> for UdpSource {
    type Error = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.poll_send_to(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: UdpPacket) -> Result<(), Self::Error> {
        self.send_state = SendState::Sending(item);
        Ok(())
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.poll_ready(cx)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}

impl RawUdpSource for UdpSource {}
