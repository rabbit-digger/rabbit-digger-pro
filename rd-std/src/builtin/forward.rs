use std::net::SocketAddr;

use rd_interface::{
    async_trait, registry::ServerFactory, util::connect_tcp, Arc, ConnectionPool, Context,
    IServer, IntoAddress, Net, Result, TcpListener, TcpStream,
};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ForwardConfig {
    bind: String,
    target: String,
}
pub struct ForwardNet {
    listen_net: Net,
    net: Net,
    cfg: Arc<ForwardConfig>,
}

impl ForwardNet {
    fn new(listen_net: Net, net: Net, cfg: ForwardConfig) -> ForwardNet {
        ForwardNet {
            listen_net,
            net,
            cfg: Arc::new(cfg),
        }
    }
}
#[async_trait]
impl IServer for ForwardNet {
    async fn start(&self, pool: ConnectionPool) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), self.cfg.bind.into_address()?)
            .await?;
        self.serve_listener(pool, listener).await
    }
}

impl ForwardNet {
    async fn serve_connection(
        cfg: Arc<ForwardConfig>,
        socket: TcpStream,
        net: Net,
        addr: SocketAddr,
    ) -> Result<()> {
        let target = net
            .tcp_connect(
                &mut Context::from_socketaddr(addr),
                cfg.target.into_address()?,
            )
            .await?;
        connect_tcp(socket, target).await?;
        Ok(())
    }
    pub async fn serve_listener(&self, pool: ConnectionPool, listener: TcpListener) -> Result<()> {
        loop {
            let (socket, addr) = listener.accept().await?;
            let cfg = self.cfg.clone();
            let net = self.net.clone();
            let _ = pool.spawn(async move {
                if let Err(e) = Self::serve_connection(cfg, socket, net, addr).await {
                    log::error!("Error when serve_connection: {:?}", e);
                }
            });
        }
    }
}

impl ServerFactory for ForwardNet {
    const NAME: &'static str = "forward";

    type Config = ForwardConfig;

    fn new(listen_net: Net, net: Net, config: Self::Config) -> Result<Self> {
        Ok(ForwardNet::new(listen_net, net, config))
    }
}
