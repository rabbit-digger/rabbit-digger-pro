use std::net::SocketAddr;

use crate::protocol::{Channel, CommandRequest, CommandResponse, Protocol};
use rd_interface::{
    async_trait, Address, Arc, Error, INet, ITcpListener, IntoDyn, Result, TcpListener, TcpStream,
    UdpSocket, NOT_IMPLEMENTED,
};
use tokio::sync::Mutex;

pub struct RemoteNet {
    protocol: Arc<dyn Protocol>,
}

pub struct RemoteListener {
    protocol: Arc<dyn Protocol>,
    channel: Mutex<Channel>,
    addr: SocketAddr,
}

#[async_trait]
impl ITcpListener for RemoteListener {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        let mut channel = self.channel.lock().await;
        let resp = channel.recv().await?;
        let (conn, addr) = match resp {
            CommandResponse::Accept { id, addr } => {
                let mut conn = self.protocol.channel().await?;
                conn.send(CommandRequest::TcpAccept { id }).await?;
                (conn, addr)
            }
            _ => return Err(Error::Other("Invalid response".into())),
        };

        Ok((conn.into_inner(), addr))
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.addr)
    }
}

#[async_trait]
impl INet for RemoteNet {
    async fn tcp_connect(
        &self,
        _ctx: &mut rd_interface::Context,
        address: Address,
    ) -> Result<TcpStream> {
        let mut channel = self.protocol.channel().await?;

        channel.send(CommandRequest::TcpConnect { address }).await?;

        Ok(channel.into_inner())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        address: Address,
    ) -> Result<TcpListener> {
        let mut channel = self.protocol.channel().await?;

        channel.send(CommandRequest::TcpBind { address }).await?;
        let resp = channel.recv().await?;
        let addr = match resp {
            CommandResponse::BindAddr { addr } => addr,
            _ => return Err(Error::Other("Invalid response".into())),
        };

        Ok(RemoteListener {
            protocol: self.protocol.clone(),
            channel: Mutex::new(channel),
            addr,
        }
        .into_dyn())
    }

    async fn udp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        _addr: Address,
    ) -> Result<UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
}

impl RemoteNet {
    pub fn new(protocol: Arc<dyn Protocol>) -> RemoteNet {
        RemoteNet { protocol }
    }
}
