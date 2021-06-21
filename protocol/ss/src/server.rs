use std::net::SocketAddr;

use super::wrapper::{Cipher, CryptoStream};
use rd_interface::{
    async_trait, prelude::*, util::connect_tcp, Address, Arc, IServer, Net, Result, TcpStream,
};
use shadowsocks::{config::ServerType, context::Context, ServerConfig};
use socks5_protocol::Address as S5Addr;

#[rd_config]
#[derive(Debug, Clone)]
pub struct SSServerConfig {
    bind: Address,
    password: String,
    #[serde(default)]
    udp: bool,

    cipher: Cipher,
}

pub struct SSServer {
    context: Arc<Context>,
    cfg: Arc<SSServerConfig>,
    listen: Net,
    net: Net,
}

#[async_trait]
impl IServer for SSServer {
    async fn start(&self) -> Result<()> {
        let listener = self
            .listen
            .tcp_bind(&mut rd_interface::Context::new(), self.cfg.bind.clone())
            .await?;
        loop {
            let (socket, addr) = listener.accept().await?;
            let cfg = self.cfg.clone();
            let context = self.context.clone();
            let net = self.net.clone();
            let _ = tokio::spawn(async move {
                if let Err(e) = Self::serve_connection(cfg, context, socket, net, addr).await {
                    tracing::error!("Error when serve_connection: {:?}", e);
                }
            });
        }
    }
}

impl SSServer {
    pub fn new(listen: Net, net: Net, cfg: SSServerConfig) -> SSServer {
        let context = Arc::new(Context::new(ServerType::Local));
        SSServer {
            context,
            cfg: Arc::new(cfg),
            listen,
            net,
        }
    }
    async fn serve_connection(
        cfg: Arc<SSServerConfig>,
        context: Arc<Context>,
        socket: TcpStream,
        net: Net,
        addr: SocketAddr,
    ) -> Result<()> {
        let svr_cfg =
            ServerConfig::new(("example.com", 0), cfg.password.clone(), cfg.cipher.into());
        let mut socket =
            CryptoStream::from_stream(context, socket, cfg.cipher.into(), svr_cfg.key());
        let target = S5Addr::read(&mut socket).await.map_err(|e| e.to_io_err())?;

        let target = net
            .tcp_connect(
                &mut rd_interface::Context::from_socketaddr(addr),
                match target {
                    S5Addr::Domain(d, p) => Address::Domain(d, p),
                    S5Addr::SocketAddr(s) => Address::SocketAddr(s),
                },
            )
            .await?;
        connect_tcp(socket, target).await?;
        Ok(())
    }
}
