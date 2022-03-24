use std::net::SocketAddr;

use rd_derive::rd_config;
use rd_interface::{async_trait, prelude::*, registry::Builder, Address, Error, INet, Net, Result};
use trust_dns_resolver::{
    config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts},
    TokioAsyncResolver,
};

#[rd_config]
#[derive(Debug)]
pub enum DnsServer {
    Google,
    Cloudflare,
    Custom { nameserver: Vec<SocketAddr> },
}
#[rd_config]
#[derive(Debug)]
pub struct DnsConfig {
    server: DnsServer,
}

pub struct DnsNet {
    resolver: TokioAsyncResolver,
}

#[async_trait]
impl INet for DnsNet {
    async fn lookup_host(&self, addr: &Address) -> Result<Vec<SocketAddr>> {
        // TODO: is it cheap?
        let r = self.resolver.clone();
        addr.resolve(move |host, port| async move {
            let response = r.lookup_ip(host).await?;

            Ok(response
                .into_iter()
                .map(|ip| SocketAddr::new(ip, port))
                .collect())
        })
        .await
        .map_err(Into::into)
    }
}

impl Builder<Net> for DnsNet {
    const NAME: &'static str = "dns";
    type Config = DnsConfig;
    type Item = Self;

    fn build(config: Self::Config) -> Result<Self> {
        let resolver = match config.server {
            DnsServer::Google => {
                TokioAsyncResolver::tokio(ResolverConfig::google(), ResolverOpts::default())
            }
            DnsServer::Cloudflare => {
                TokioAsyncResolver::tokio(ResolverConfig::cloudflare(), ResolverOpts::default())
            }
            DnsServer::Custom { nameserver } => {
                let groups = nameserver
                    .into_iter()
                    .map(|s| nameserver_config(s))
                    .collect::<Vec<_>>();
                TokioAsyncResolver::tokio(
                    ResolverConfig::from_parts(None, vec![], groups),
                    ResolverOpts::default(),
                )
            }
        }
        .map_err(|e| Error::other(format!("Failed to build resolver: {:?}", e)))?;

        Ok(Self { resolver })
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
