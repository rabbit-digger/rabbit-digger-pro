mod address;
pub mod context;
mod error;
mod interface;
pub mod registry;

pub use address::{Address, IntoAddress};
pub use context::Context;
pub use error::{Error, Result, NOT_IMPLEMENTED};
use futures_util::future::BoxFuture;
pub use interface::*;
pub use registry::Registry;
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
    #[inline(always)]
    fn tcp_connect<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<TcpStream>>
    where
        Self: 'a,
    {
        self.tcp_connect.tcp_connect(ctx, addr)
    }

    #[inline(always)]
    fn tcp_bind<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<TcpListener>>
    where
        Self: 'a,
    {
        self.tcp_bind.tcp_bind(ctx, addr)
    }

    #[inline(always)]
    fn udp_bind<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<UdpSocket>>
    where
        Self: 'a,
    {
        self.udp_bind.udp_bind(ctx, addr)
    }
}
