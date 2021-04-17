mod util;

#[cfg(target_os = "linux")]
use linux::{RedirServer, RedirServerConfig};
use rd_interface::{config::from_value, Registry, Result};

#[cfg(target_os = "linux")]
mod linux {
    use std::net::SocketAddr;

    use crate::util::OriginAddrExt;
    use async_std::{
        net::{TcpListener, TcpStream},
        task::spawn,
    };
    use rd_interface::{
        async_trait, context::common_field::SourceAddress, util::connect_tcp, Context, IServer,
        IntoAddress, Net, Result,
    };
    use serde_derive::Deserialize;

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

        async fn stop(&self) -> Result<()> {
            // TODO
            Ok(())
        }
    }

    async fn new_context(addr: SocketAddr) -> Context {
        let mut ctx = Context::new();
        let _ = ctx
            .insert_common::<SourceAddress>(SourceAddress { addr })
            .await
            .ok();
        ctx
    }

    impl RedirServer {
        pub fn new(cfg: RedirServerConfig, net: Net) -> Self {
            RedirServer { cfg, net }
        }

        pub async fn serve_listener(&self, listener: TcpListener) -> Result<()> {
            loop {
                let (socket, addr) = listener.accept().await?;
                let _ = spawn(Self::serve_connection(self.net.clone(), socket, addr));
            }
        }

        async fn serve_connection(net: Net, socket: TcpStream, addr: SocketAddr) -> Result<()> {
            let target = socket.origin_addr()?;

            let target_tcp = net
                .tcp_connect(&mut new_context(addr).await, target.into_address()?)
                .await?;

            connect_tcp(socket, target_tcp).await?;

            Ok(())
        }
    }
}

#[no_mangle]
pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    #[cfg(target_os = "linux")]
    registry.add_server("redir", |_listen_net, net, cfg| {
        let cfg: RedirServerConfig = from_value(cfg)?;
        Ok(RedirServer::new(cfg, net))
    });
    Ok(())
}
