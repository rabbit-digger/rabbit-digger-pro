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

use self::rd_runtime::{TokioConnection, TokioConnectionProvider, TokioHandle};

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
    resolver: AsyncResolver<TokioConnection, TokioConnectionProvider>,
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
            TokioHandle(net.clone()),
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
    use std::task::Poll;

    use super::*;
    use futures::{ready, Future};
    use parking_lot::Mutex;
    use trust_dns_resolver::{
        name_server::{GenericConnection, GenericConnectionProvider, RuntimeProvider, Spawn},
        proto::{error::ProtoError, iocompat::AsyncIoTokioAsStd, udp::UdpSocket, TokioTime},
    };

    tokio::task_local! {
        pub(super) static NET:  Net;
    }

    pub struct RDUdpSocket(Mutex<rd_interface::UdpSocket>);

    #[async_trait]
    impl UdpSocket for RDUdpSocket {
        type Time = TokioTime;

        async fn bind(addr: SocketAddr) -> std::io::Result<Self> {
            let net = NET.with(Clone::clone);

            net.udp_bind(&mut rd_interface::Context::new(), &addr.into())
                .await
                .map(Mutex::new)
                .map(RDUdpSocket)
                .map_err(|e| e.to_io_err())
        }

        fn poll_recv_from(
            &self,
            cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> Poll<std::io::Result<(usize, SocketAddr)>> {
            let mut buf = tokio::io::ReadBuf::new(buf);
            let addr = ready!(self.0.lock().poll_recv_from(cx, &mut buf))?;
            let len = buf.filled().len();

            Poll::Ready(Ok((len, addr)))
        }

        fn poll_send_to(
            &self,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
            target: SocketAddr,
        ) -> Poll<std::io::Result<usize>> {
            self.0.lock().poll_send_to(cx, buf, &target.into())
        }
    }

    #[derive(Clone)]
    pub struct TokioHandle(pub Net);
    impl Spawn for TokioHandle {
        fn spawn_bg<F>(&mut self, future: F)
        where
            F: Future<Output = Result<(), ProtoError>> + Send + 'static,
        {
            let net = self.0.clone();
            let _join = tokio::spawn(async move { NET.scope(net, future).await });
        }
    }

    #[derive(Clone, Copy)]
    pub struct TokioRuntime;
    impl RuntimeProvider for TokioRuntime {
        type Handle = TokioHandle;
        type Tcp = AsyncIoTokioAsStd<tokio::net::TcpStream>;
        type Timer = TokioTime;
        type Udp = RDUdpSocket;
    }
    pub type TokioConnection = GenericConnection;
    pub type TokioConnectionProvider = GenericConnectionProvider<TokioRuntime>;
}
