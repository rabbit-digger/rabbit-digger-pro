use std::net::SocketAddr;

use rd_interface::{
    async_trait, impl_async_read_write, impl_stream_sink, ITcpListener, ITcpStream, IUdpSocket,
    IntoDyn, Result,
};
use tokio::sync::Mutex;
use tokio_smoltcp::{TcpListener, TcpSocket, UdpSocket};

pub struct TcpStreamWrap(pub(crate) TcpSocket);
impl_async_read_write!(TcpStreamWrap, 0);

#[async_trait]
impl ITcpStream for TcpStreamWrap {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.peer_addr()?)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.local_addr()?)
    }
}

pub struct TcpListenerWrap(pub(crate) Mutex<TcpListener>, pub(crate) SocketAddr);

#[async_trait]
impl ITcpListener for TcpListenerWrap {
    async fn accept(&self) -> Result<(rd_interface::TcpStream, SocketAddr)> {
        let (tcp, addr) = self.0.lock().await.accept().await?;
        Ok((TcpStreamWrap(tcp).into_dyn(), addr))
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.1)
    }
}

pub struct UdpSocketWrap(pub(crate) UdpSocket);

impl_stream_sink!(UdpSocketWrap, 0);

#[async_trait]
impl IUdpSocket for UdpSocketWrap {
    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.local_addr()?)
    }
}
