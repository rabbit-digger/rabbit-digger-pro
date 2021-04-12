use std::net::SocketAddr;

use async_std::task::spawn;
use rd_interface::{
    async_trait, config::from_value, context::common_field::SourceAddress, util::connect_tcp, Arc,
    Context, IServer, IntoAddress, Net, Registry, Result, TcpListener, TcpStream,
};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
struct ForwardConfig {
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
    async fn start(&self) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), self.cfg.bind.into_address()?)
            .await?;
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

impl ForwardNet {
    async fn serve_connection(
        cfg: Arc<ForwardConfig>,
        socket: TcpStream,
        net: Net,
        addr: SocketAddr,
    ) -> Result<()> {
        let target = net
            .tcp_connect(&mut new_context(addr).await, cfg.target.into_address()?)
            .await?;
        connect_tcp(socket, target).await?;
        Ok(())
    }
    pub async fn serve_listener(&self, listener: TcpListener) -> Result<()> {
        loop {
            let (socket, addr) = listener.accept().await?;
            let cfg = self.cfg.clone();
            let net = self.net.clone();
            let _ = spawn(Self::serve_connection(cfg, socket, net, addr));
        }
    }
}

pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_server("forward", |listen_net, net, cfg| {
        let cfg: ForwardConfig = from_value(cfg)?;
        Ok(ForwardNet::new(listen_net, net, cfg))
    });
    Ok(())
}
