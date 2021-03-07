mod address;
mod context;
mod error;
mod interface;
mod registry;

pub use address::{Address, IntoAddress};
pub use context::Context;
pub use error::{Error, Result, NOT_IMPLEMENTED};
pub use interface::*;
pub use registry::{NetFromConfig, Registry, ServerFromConfig};
pub mod config {
    pub use serde_json::{self, from_value, Error, Value};
}

/// A no-op Net returns [`Error::NotImplemented`](crate::Error::NotImplemented) for every method.
pub struct NotImplementedNet;

#[async_trait]
impl INet for NotImplementedNet {
    async fn tcp_connect(&self, _ctx: &mut Context, _addr: Address) -> Result<TcpStream> {
        Err(NOT_IMPLEMENTED)
    }

    async fn tcp_bind(&self, _ctx: &mut Context, _addr: Address) -> Result<TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(&self, _ctx: &mut Context, _addr: Address) -> Result<UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
}

/// A new Net calls [`tcp_connect()`](crate::INet::tcp_connect()), [`tcp_bind()`](crate::INet::tcp_bind()), [`udp_bind()`](crate::INet::udp_bind()) from different Net.
pub struct CombineNet {
    pub tcp_connect: Net,
    pub tcp_bind: Net,
    pub udp_bind: Net,
}

#[async_trait]
impl INet for CombineNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: Address) -> Result<TcpStream> {
        self.tcp_connect.tcp_connect(ctx, addr).await
    }

    async fn tcp_bind(&self, ctx: &mut Context, addr: Address) -> Result<TcpListener> {
        self.tcp_bind.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: Address) -> Result<UdpSocket> {
        self.udp_bind.udp_bind(ctx, addr).await
    }
}
