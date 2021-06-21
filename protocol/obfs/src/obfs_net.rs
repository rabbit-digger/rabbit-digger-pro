use std::net::SocketAddr;

use crate::{Obfs, ObfsType};
use rd_interface::{
    async_trait, prelude::*, registry::NetRef, Address, Arc, Context, INet, ITcpListener, IntoDyn,
    Net, Result, TcpListener, TcpStream, UdpSocket, NOT_IMPLEMENTED,
};

type BoxObfs = Arc<dyn Obfs + Send + Sync + 'static>;

#[rd_config]
#[derive(Debug)]
pub struct ObfsNetConfig {
    #[serde(default)]
    pub net: NetRef,
    #[serde(default, flatten)]
    pub obfs_type: ObfsType,
}

pub struct ObfsNet {
    net: Net,
    obfs: Arc<ObfsType>,
}

impl ObfsNet {
    pub fn new(config: ObfsNetConfig) -> Result<Self> {
        Ok(ObfsNet {
            net: config.net.net(),
            obfs: Arc::new(config.obfs_type),
        })
    }
}

#[async_trait]
impl INet for ObfsNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: Address) -> Result<TcpStream> {
        let tcp = self.net.tcp_connect(ctx, addr.clone()).await?;
        Ok(self.obfs.tcp_connect(tcp, ctx, addr)?)
    }

    async fn tcp_bind(&self, ctx: &mut Context, addr: Address) -> Result<TcpListener> {
        let listener = self.net.tcp_bind(ctx, addr).await?;
        Ok(ObfsTcpListener(listener, self.obfs.clone()).into_dyn())
    }

    async fn udp_bind(&self, _ctx: &mut Context, _addr: Address) -> Result<UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
}

struct ObfsTcpListener(TcpListener, BoxObfs);

#[async_trait]
impl ITcpListener for ObfsTcpListener {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        let (tcp, addr) = self.0.accept().await?;
        Ok((self.1.tcp_accept(tcp, addr)?, addr))
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().await
    }
}
