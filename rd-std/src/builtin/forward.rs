use std::net::SocketAddr;

use crate::util::connect_tcp;
use rd_interface::{
    async_trait, prelude::*, registry::ServerFactory, Address, Arc, Context, IServer, Net, Result,
    TcpListener, TcpStream,
};

/// A server that forwards all connections to target.
#[rd_config]
#[derive(Debug)]
pub struct ForwardServerConfig {
    bind: Address,
    target: Address,
}

pub struct ForwardServer {
    listen_net: Net,
    net: Net,
    cfg: Arc<ForwardServerConfig>,
}

impl ForwardServer {
    fn new(listen_net: Net, net: Net, cfg: ForwardServerConfig) -> ForwardServer {
        ForwardServer {
            listen_net,
            net,
            cfg: Arc::new(cfg),
        }
    }
}
#[async_trait]
impl IServer for ForwardServer {
    async fn start(&self) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), &self.cfg.bind)
            .await?;
        self.serve_listener(listener).await
    }
}

impl ForwardServer {
    async fn serve_connection(
        cfg: Arc<ForwardServerConfig>,
        socket: TcpStream,
        net: Net,
        addr: SocketAddr,
    ) -> Result<()> {
        let ctx = &mut Context::from_socketaddr(addr);
        let target = net.tcp_connect(ctx, &cfg.target).await?;
        connect_tcp(ctx, socket, target).await?;
        Ok(())
    }
    pub async fn serve_listener(&self, listener: TcpListener) -> Result<()> {
        loop {
            let (socket, addr) = listener.accept().await?;
            let cfg = self.cfg.clone();
            let net = self.net.clone();
            let _ = tokio::spawn(async move {
                if let Err(e) = Self::serve_connection(cfg, socket, net, addr).await {
                    tracing::error!("Error when serve_connection: {:?}", e);
                }
            });
        }
    }
}

impl ServerFactory for ForwardServer {
    const NAME: &'static str = "forward";
    type Config = ForwardServerConfig;
    type Server = Self;

    fn new(listen: Net, net: Net, cfg: Self::Config) -> Result<Self> {
        Ok(ForwardServer::new(listen, net, cfg))
    }
}
