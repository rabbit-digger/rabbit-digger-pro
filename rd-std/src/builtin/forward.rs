use std::net::SocketAddr;

use crate::util::connect_tcp;
use rd_interface::{
    async_trait, prelude::*, registry::ServerBuilder, Address, Arc, Context, IServer, Net, Result,
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

impl ServerBuilder for ForwardServer {
    const NAME: &'static str = "forward";
    type Config = ForwardServerConfig;
    type Server = Self;

    fn build(listen: Net, net: Net, cfg: Self::Config) -> Result<Self> {
        Ok(ForwardServer::new(listen, net, cfg))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rd_interface::{IntoAddress, IntoDyn};
    use tokio::time::sleep;

    use super::*;
    use crate::tests::{assert_echo, spawn_echo_server, TestNet};

    #[tokio::test]
    async fn test_forward_server() {
        let net = TestNet::new().into_dyn();
        let cfg = ForwardServerConfig {
            bind: "127.0.0.1:1234".into_address().unwrap(),
            target: "127.0.0.1:4321".into_address().unwrap(),
        };
        let server = ForwardServer::new(net.clone(), net.clone(), cfg);
        tokio::spawn(async move { server.start().await.unwrap() });
        spawn_echo_server(&net, "127.0.0.1:4321").await;

        sleep(Duration::from_millis(1)).await;

        assert_echo(&net, "127.0.0.1:1234").await;
    }
}
