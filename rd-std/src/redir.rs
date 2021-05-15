mod origin_addr;

#[cfg(target_os = "linux")]
use linux::RedirServer;
use rd_interface::{Registry, Result};

#[cfg(target_os = "linux")]
mod linux {
    use std::net::SocketAddr;

    use super::origin_addr::OriginAddrExt;
    use crate::builtin::local::CompatTcp;
    use rd_interface::{
        async_trait, registry::ServerFactory, util::connect_tcp, Context, IServer, IntoAddress,
        IntoDyn, Net, Result,
    };
    use serde_derive::Deserialize;
    use tokio::net::{TcpListener, TcpStream};

    #[derive(Debug, Deserialize)]
    pub struct RedirServerConfig {
        bind: String,
    }

    pub struct RedirServer {
        cfg: RedirServerConfig,
        net: Net,
    }

    #[async_trait]
    impl IServer for RedirServer {
        async fn start(&self) -> Result<()> {
            let listener = TcpListener::bind(&self.cfg.bind).await?;
            self.serve_listener(listener).await
        }
    }

    impl RedirServer {
        pub fn new(cfg: RedirServerConfig, net: Net) -> Self {
            RedirServer { cfg, net }
        }

        pub async fn serve_listener(&self, listener: TcpListener) -> Result<()> {
            loop {
                let (socket, addr) = listener.accept().await?;
                let net = self.net.clone();
                let _ = tokio::spawn(async move {
                    if let Err(e) = Self::serve_connection(net, socket, addr).await {
                        log::error!("Error when serve_connection: {:?}", e);
                    }
                });
            }
        }

        async fn serve_connection(net: Net, socket: TcpStream, addr: SocketAddr) -> Result<()> {
            let target = socket.origin_addr()?;

            let target_tcp = net
                .tcp_connect(&mut Context::from_socketaddr(addr), target.into_address()?)
                .await?;
            let socket = CompatTcp(socket).into_dyn();

            connect_tcp(socket, target_tcp).await?;

            Ok(())
        }
    }

    impl ServerFactory for RedirServer {
        const NAME: &'static str = "redir";
        type Config = RedirServerConfig;
        type Server = Self;

        fn new(_: Net, net: Net, config: Self::Config) -> Result<Self> {
            Ok(RedirServer::new(config, net))
        }
    }
}

pub fn init(_registry: &mut Registry) -> Result<()> {
    #[cfg(target_os = "linux")]
    _registry.add_server::<RedirServer>();
    Ok(())
}
