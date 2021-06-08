use std::net::SocketAddr;

use crate::Obfs;
use rd_interface::{
    async_trait, impl_async_read_write,
    registry::NetRef,
    schemars::{self, JsonSchema},
    Address, Config, Context, INet, ITcpListener, ITcpStream, IntoDyn, Net, Result, TcpListener,
    TcpStream, UdpSocket, NOT_IMPLEMENTED,
};
use serde_derive::{Deserialize, Serialize};

type BoxObfs = Box<dyn Obfs + Send + Sync>;

#[derive(Debug, Serialize, Deserialize, Config, JsonSchema)]
pub struct ObfsNetConfig {
    #[serde(default)]
    net: NetRef,
}

#[derive(Clone)]
struct ObfsFactory;

impl ObfsFactory {
    fn get_obfs(&self) -> BoxObfs {
        todo!()
    }
}

pub struct ObfsNet {
    net: Net,
    factory: ObfsFactory,
}

impl ObfsNet {
    pub fn new(config: ObfsNetConfig) -> Result<Self> {
        Ok(ObfsNet {
            net: config.net.net(),
            factory: ObfsFactory,
        })
    }
}

struct ConnectTcpStream(TcpStream, BoxObfs);
impl_async_read_write!(ConnectTcpStream, 0);

#[async_trait]
impl ITcpStream for ConnectTcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        self.0.peer_addr().await
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().await
    }
}

struct AcceptTcpStream(TcpStream, BoxObfs);
impl_async_read_write!(AcceptTcpStream, 0);

#[async_trait]
impl ITcpStream for AcceptTcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        self.0.peer_addr().await
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().await
    }
}

#[async_trait]
impl INet for ObfsNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: Address) -> Result<TcpStream> {
        let tcp = self.net.tcp_connect(ctx, addr).await?;
        Ok(ConnectTcpStream(tcp, self.factory.get_obfs()).into_dyn())
    }

    async fn tcp_bind(&self, ctx: &mut Context, addr: Address) -> Result<TcpListener> {
        let listener = self.net.tcp_bind(ctx, addr).await?;
        Ok(ObfsTcpListener(listener, self.factory.clone()).into_dyn())
    }

    async fn udp_bind(&self, _ctx: &mut Context, _addr: Address) -> Result<UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
}

struct ObfsTcpListener(TcpListener, ObfsFactory);

#[async_trait]
impl ITcpListener for ObfsTcpListener {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        let (tcp, addr) = self.0.accept().await?;
        Ok((AcceptTcpStream(tcp, self.1.get_obfs()).into_dyn(), addr))
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().await
    }
}
