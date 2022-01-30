use rd_interface::{
    async_trait, prelude::*, registry::ServerFactory, Address, Context, IServer, Net, Result,
    TcpListener, TcpStream,
};
use tokio::io;

/// A echo server.
#[rd_config]
#[derive(Debug)]
pub struct EchoServerConfig {
    bind: Address,
}

pub struct EchoServer {
    listen_net: Net,
    bind: Address,
}

impl EchoServer {
    fn new(listen_net: Net, EchoServerConfig { bind }: EchoServerConfig) -> EchoServer {
        EchoServer { listen_net, bind }
    }
}
#[async_trait]
impl IServer for EchoServer {
    async fn start(&self) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), &self.bind)
            .await?;
        self.serve_listener(listener).await
    }
}

impl EchoServer {
    async fn serve_connection(socket: TcpStream) -> Result<()> {
        let (mut rx, mut tx) = io::split(socket);
        io::copy(&mut rx, &mut tx).await?;
        Ok(())
    }
    pub async fn serve_listener(&self, listener: TcpListener) -> Result<()> {
        loop {
            let (socket, _) = listener.accept().await?;
            let _ = tokio::spawn(async move {
                if let Err(e) = Self::serve_connection(socket).await {
                    tracing::error!("Error when serve_connection: {:?}", e);
                }
            });
        }
    }
}

impl ServerFactory for EchoServer {
    const NAME: &'static str = "echo";
    type Config = EchoServerConfig;
    type Server = Self;

    fn new(listen: Net, _net: Net, cfg: Self::Config) -> Result<Self> {
        Ok(EchoServer::new(listen, cfg))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rd_interface::{IntoAddress, IntoDyn};
    use tokio::time::sleep;

    use super::*;
    use crate::tests::{assert_echo, TestNet};

    #[tokio::test]
    async fn test_echo_server() {
        let net = TestNet::new().into_dyn();
        let cfg = EchoServerConfig {
            bind: "127.0.0.1:1234".into_address().unwrap(),
        };
        let server = EchoServer::new(net.clone(), cfg);
        tokio::spawn(async move { server.start().await.unwrap() });

        sleep(Duration::from_millis(1)).await;

        assert_echo(&net, "127.0.0.1:1234").await;
    }
}
