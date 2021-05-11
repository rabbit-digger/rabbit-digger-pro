use super::protocol::{
    AuthMethod, AuthRequest, AuthResponse, CommandRequest, CommandResponse, Version,
};

use super::common::{pack_udp, parse_udp, Address};
use rd_interface::{
    async_trait, error::map_other, impl_async_read_write, INet, ITcpStream, IUdpSocket,
    IntoAddress, IntoDyn, Net, Result, TcpStream, UdpSocket, NOT_IMPLEMENTED,
};
use std::net::SocketAddr;
use tokio::io::{split, AsyncWriteExt, BufWriter};

pub struct Socks5Client {
    address: String,
    port: u16,
    net: Net,
}

pub struct Socks5TcpStream(TcpStream);

impl_async_read_write!(Socks5TcpStream, 0);

pub struct Socks5UdpSocket(UdpSocket, TcpStream, SocketAddr);

#[async_trait]
impl IUdpSocket for Socks5UdpSocket {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        // 259 is max size of address, atype 1 + domain len 1 + domain 255 + port 2
        let bytes_size = 259 + buf.len();
        let mut bytes = vec![0u8; bytes_size];
        let recv_len = loop {
            let (len, addr) = self.0.recv_from(&mut bytes).await?;
            if addr == self.2 {
                break len;
            }
        };
        bytes.truncate(recv_len);

        let (addr, payload) = parse_udp(&bytes).await?;
        let to_copy = payload.len().min(buf.len());
        buf[..to_copy].copy_from_slice(&payload[..to_copy]);

        Ok((to_copy, addr.to_socket_addr()?))
    }

    async fn send_to(&self, buf: &[u8], addr: rd_interface::Address) -> Result<usize> {
        let addr: Address = addr.into();

        let bytes = pack_udp(addr, buf).await?;

        self.0.send_to(&bytes, self.2.into()).await
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().await
    }
}

#[async_trait]
impl ITcpStream for Socks5TcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}

#[async_trait]
impl INet for Socks5Client {
    async fn udp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: rd_interface::Address,
    ) -> Result<UdpSocket> {
        let mut socket = self.net.tcp_connect(ctx, self.server()?).await?;

        let req = CommandRequest::udp_associate(addr.clone().into_address()?.into());
        let resp = self.send_command(&mut socket, req).await?;
        let client = self.net.udp_bind(ctx, addr.clone()).await?;

        let addr = resp.address.to_socket_addr()?;

        Ok(Socks5UdpSocket(client, socket, addr).into_dyn())
    }
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: rd_interface::Address,
    ) -> Result<TcpStream> {
        let mut socket = self.net.tcp_connect(ctx, self.server()?).await?;

        let req = CommandRequest::connect(addr.into_address()?.into());
        let _resp = self.send_command(&mut socket, req).await?;

        Ok(Socks5TcpStream(socket).into_dyn())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        _addr: rd_interface::Address,
    ) -> Result<rd_interface::TcpListener> {
        Err(rd_interface::Error::NotImplemented)
    }
}

impl Socks5Client {
    pub fn new(net: Net, address: String, port: u16) -> Self {
        Self { address, port, net }
    }
    fn server(&self) -> Result<rd_interface::Address> {
        (self.address.as_str(), self.port)
            .into_address()
            .map_err(Into::into)
    }
    async fn send_command(
        &self,
        socket: &mut TcpStream,
        command_req: CommandRequest,
    ) -> Result<CommandResponse> {
        let (mut rx, tx) = split(socket);
        let mut tx = BufWriter::with_capacity(512, tx);

        let version = Version::V5;
        let auth_req = AuthRequest::new(vec![AuthMethod::Noauth]);
        version.write(&mut tx).await.map_err(map_other)?;
        auth_req.write(&mut tx).await.map_err(map_other)?;
        tx.flush().await?;

        Version::read(&mut rx).await.map_err(map_other)?;
        let resp = AuthResponse::read(&mut rx).await.map_err(map_other)?;
        if resp.method() != AuthMethod::Noauth {
            return Err(rd_interface::Error::Other("Auth failed".to_string().into()));
        }

        command_req.write(&mut tx).await.map_err(map_other)?;
        tx.flush().await?;

        let command_resp = CommandResponse::read(rx).await.map_err(map_other)?;

        Ok(command_resp)
    }
}
