use std::net::SocketAddr;

use super::origin_addr::OriginAddrExt;
use crate::{builtin::local::CompatTcp, util::connect_tcp};
use rd_derive::rd_config;
use rd_interface::{
    async_trait, config::NetRef, registry::ServerBuilder, schemars, Address, Context, IServer,
    IntoAddress, IntoDyn, Net, Result,
};
use tokio::net::{TcpListener, TcpStream};
use tracing::instrument;

#[rd_config]
#[derive(Debug)]
pub struct RedirServerConfig {
    bind: Address,
    #[serde(default)]
    net: NetRef,
}

pub struct RedirServer {
    bind: Address,
    net: Net,
}

#[async_trait]
impl IServer for RedirServer {
    async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.bind.to_string()).await?;
        self.serve_listener(listener).await
    }
}

impl RedirServer {
    pub fn new(bind: Address, net: Net) -> Self {
        RedirServer { bind, net }
    }

    pub async fn serve_listener(&self, listener: TcpListener) -> Result<()> {
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

    #[instrument(err, skip(net, socket))]
    async fn serve_connection(net: Net, socket: TcpStream, addr: SocketAddr) -> Result<()> {
        let target = socket.origin_addr()?;

        let ctx = &mut Context::from_socketaddr(addr);
        let target_tcp = net.tcp_connect(ctx, &target.into_address()?).await?;
        let socket = CompatTcp(socket).into_dyn();

        connect_tcp(ctx, socket, target_tcp).await?;

        Ok(())
    }
}

impl ServerBuilder for RedirServer {
    const NAME: &'static str = "redir";
    type Config = RedirServerConfig;
    type Server = Self;

    fn build(Self::Config { bind, net }: Self::Config) -> Result<Self> {
        Ok(RedirServer::new(bind, (*net).clone()))
    }
}
