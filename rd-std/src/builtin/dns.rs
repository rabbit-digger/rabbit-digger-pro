use std::net::SocketAddr;

use rd_derive::rd_config;
use rd_interface::{
    async_trait, config::NetRef, prelude::*, registry::Builder, Address, Error, INet, IntoDyn, Net,
    Result,
};
use trust_dns_resolver::{
    config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts},
    AsyncResolver,
};

use self::rd_runtime::{RDConnection, RDConnectionProvider, RDHandle};

use super::local::{LocalNet, LocalNetConfig};

/// A net refering to another net.
#[rd_config]
#[derive(Debug)]
#[serde(rename_all = "camelCase")]
pub enum DnsServer {
    Google,
    Cloudflare,
    Custom { nameserver: Vec<SocketAddr> },
}

#[rd_config]
#[derive(Debug)]
pub struct DnsConfig {
    server: DnsServer,
    #[serde(default)]
    net: Option<NetRef>,
}

pub struct DnsNet {
    net: Net,
    resolver: AsyncResolver<RDConnection, RDConnectionProvider>,
}

#[async_trait]
impl INet for DnsNet {
    async fn lookup_host(&self, addr: &Address) -> Result<Vec<SocketAddr>> {
        // TODO: is it cheap?
        let r = self.resolver.clone();
        rd_runtime::NET
            .scope(self.net.clone(), async move {
                addr.resolve(move |host, port| async move {
                    let response = r.lookup_ip(host).await?;

                    Ok(response
                        .into_iter()
                        .map(|ip| SocketAddr::new(ip, port))
                        .collect())
                })
                .await
                .map_err(Into::into)
            })
            .await
    }
}

impl Builder<Net> for DnsNet {
    const NAME: &'static str = "dns";
    type Config = DnsConfig;
    type Item = Self;

    fn build(config: Self::Config) -> Result<Self> {
        let net = config
            .net
            .map(|i| (*i).clone())
            .unwrap_or_else(|| LocalNet::new(LocalNetConfig::default()).into_dyn());
        let resolver_config = match config.server {
            DnsServer::Google => ResolverConfig::google(),
            DnsServer::Cloudflare => ResolverConfig::cloudflare(),
            DnsServer::Custom { nameserver } => ResolverConfig::from_parts(
                None,
                vec![],
                nameserver
                    .into_iter()
                    .map(|s| nameserver_config(s))
                    .collect::<Vec<_>>(),
            ),
        };
        let resolver = AsyncResolver::new(
            resolver_config,
            ResolverOpts::default(),
            RDHandle(net.clone()),
        )
        .map_err(|e| Error::other(format!("Failed to build resolver: {:?}", e)))?;

        Ok(Self { resolver, net })
    }
}

fn nameserver_config(socket_addr: SocketAddr) -> NameServerConfig {
    NameServerConfig {
        socket_addr,
        protocol: Protocol::Udp,
        tls_dns_name: None,
        trust_nx_responses: false,
        bind_addr: None,
    }
}

mod rd_runtime {
    use std::{
        io,
        pin::Pin,
        task::{Context, Poll},
    };

    use super::*;
    use futures::{ready, Future, TryFutureExt};
    use parking_lot::Mutex;
    use trust_dns_resolver::{
        name_server::{GenericConnection, GenericConnectionProvider, RuntimeProvider, Spawn},
        proto::{error::ProtoError, iocompat::AsyncIoTokioAsStd, udp::UdpSocket, TokioTime},
    };

    tokio::task_local! {
        pub(super) static NET:  Net;
    }

    enum Inner {
        Connecting(Pin<Box<dyn Future<Output = io::Result<rd_interface::UdpSocket>> + Send>>),
        Connected(rd_interface::UdpSocket),
    }

    // TODO: remove this workaround: https://github.com/bluejekyll/trust-dns/issues/1669
    pub struct RDUdpSocket {
        inner: Mutex<Inner>,
    }

    impl RDUdpSocket {
        fn poll_udp<F, R>(&self, cx: &mut Context<'_>, f: F) -> Poll<io::Result<R>>
        where
            F: FnOnce(&mut Context<'_>, &mut rd_interface::UdpSocket) -> Poll<io::Result<R>>,
        {
            loop {
                let inner = &mut *self.inner.lock();
                match inner {
                    Inner::Connecting(fut) => {
                        let socket = ready!(fut.as_mut().poll(cx))?;
                        *inner = Inner::Connected(socket);
                    }
                    Inner::Connected(ref mut socket) => return f(cx, socket),
                }
            }
        }
    }

    #[async_trait]
    impl UdpSocket for RDUdpSocket {
        type Time = TokioTime;

        async fn bind(addr: SocketAddr) -> io::Result<Self> {
            let net = NET.with(Clone::clone);
            let fut = async move {
                net.udp_bind(&mut rd_interface::Context::new(), &addr.into())
                    .map_err(|e| e.to_io_err())
                    .await
            };
            Ok(RDUdpSocket {
                inner: Mutex::new(Inner::Connecting(Box::pin(fut))),
            })
        }

        fn poll_recv_from(
            &self,
            cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<(usize, SocketAddr)>> {
            self.poll_udp(cx, |cx, s| {
                let mut buf = tokio::io::ReadBuf::new(buf);
                let addr = ready!(s.poll_recv_from(cx, &mut buf))?;
                let len = buf.filled().len();

                Poll::Ready(Ok((len, addr)))
            })
        }

        fn poll_send_to(
            &self,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
            target: SocketAddr,
        ) -> Poll<io::Result<usize>> {
            self.poll_udp(cx, |cx, s| s.poll_send_to(cx, buf, &target.into()))
        }
    }

    #[derive(Clone)]
    pub struct RDHandle(pub Net);
    impl Spawn for RDHandle {
        fn spawn_bg<F>(&mut self, future: F)
        where
            F: Future<Output = Result<(), ProtoError>> + Send + 'static,
        {
            let net = self.0.clone();
            let _join = tokio::spawn(async move {
                if let Err(e) = NET.scope(net, future).await {
                    eprintln!("spawn return error {:?}", e);
                }
            });
        }
    }

    #[derive(Clone, Copy)]
    pub struct RDRuntime;
    impl RuntimeProvider for RDRuntime {
        type Handle = RDHandle;
        type Tcp = AsyncIoTokioAsStd<tokio::net::TcpStream>;
        type Timer = TokioTime;
        type Udp = RDUdpSocket;
    }
    pub type RDConnection = GenericConnection;
    pub type RDConnectionProvider = GenericConnectionProvider<RDRuntime>;
}
