use futures::{ready, Sink, SinkExt, Stream, StreamExt};
use socks5_protocol::{
    AuthMethod, AuthRequest, AuthResponse, CommandRequest, CommandResponse, Version,
};

use crate::socks5::common::map_err;

use super::common::{pack_udp, parse_udp, ra2sa};
use rd_interface::{
    async_trait, impl_async_read_write, Address, Bytes, BytesMut, INet, ITcpStream, IUdpSocket,
    IntoAddress, IntoDyn, Net, Result, TcpStream, UdpSocket, NOT_IMPLEMENTED,
};
use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
};
use tokio::io::{AsyncWriteExt, BufWriter};

pub struct Socks5Client {
    server: Address,
    net: Net,
}

pub struct Socks5TcpStream(TcpStream);

impl_async_read_write!(Socks5TcpStream, 0);

pub struct Socks5UdpSocket(UdpSocket, TcpStream, SocketAddr);

impl Stream for Socks5UdpSocket {
    type Item = io::Result<(BytesMut, SocketAddr)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        // 259 is max size of address, atype 1 + domain len 1 + domain 255 + port 2
        let bytes = loop {
            let (bytes, addr) = match ready!(self.0.poll_next_unpin(cx)) {
                Some(r) => r?,
                None => return Poll::Ready(None),
            };
            if addr == self.2 {
                break bytes;
            }
        };

        let (addr, payload) = parse_udp(bytes.freeze())?;

        Poll::Ready(Some(Ok((
            BytesMut::from(&payload[..]),
            addr.to_socket_addr()?,
        ))))
    }
}

impl Sink<(Bytes, Address)> for Socks5UdpSocket {
    type Error = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready_unpin(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        (buf, addr): (Bytes, Address),
    ) -> Result<(), Self::Error> {
        let Socks5UdpSocket(sink, _, server_addr) = &mut *self;
        let bytes = pack_udp(addr.into(), &buf)?;

        sink.start_send_unpin((bytes.into(), server_addr.clone().into()))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.0.poll_flush_unpin(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.0.poll_close_unpin(cx)
    }
}

#[async_trait]
impl IUdpSocket for Socks5UdpSocket {
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
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &rd_interface::Address,
    ) -> Result<TcpStream> {
        let mut socket = self.net.tcp_connect(ctx, &self.server).await?;

        let req = CommandRequest::connect(ra2sa(addr.clone().into_address()?));
        let _resp = self.send_command(&mut socket, req).await?;

        Ok(Socks5TcpStream(socket).into_dyn())
    }

    async fn udp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &rd_interface::Address,
    ) -> Result<UdpSocket> {
        let server_addr = self
            .net
            .lookup_host(&self.server)
            .await?
            .into_iter()
            .next()
            .ok_or(io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "Failed to lookup domain",
            ))?;

        let mut socket = self.net.tcp_connect(ctx, &server_addr.into()).await?;

        let req = CommandRequest::udp_associate(ra2sa(addr.clone().into_address()?));
        let resp = self.send_command(&mut socket, req).await?;
        let client = self
            .net
            .udp_bind(ctx, &rd_interface::Address::any_addr_port(&server_addr))
            .await?;

        let addr = resp.address.to_socket_addr().map_err(map_err)?;

        Ok(Socks5UdpSocket(client, socket, addr).into_dyn())
    }
}

impl Socks5Client {
    pub fn new(net: Net, server: Address) -> Self {
        Self { server, net }
    }
    async fn send_command(
        &self,
        socket: &mut TcpStream,
        command_req: CommandRequest,
    ) -> Result<CommandResponse> {
        let mut socket = BufWriter::with_capacity(512, socket);

        let version = Version::V5;
        let auth_req = AuthRequest::new(vec![AuthMethod::Noauth]);
        version.write(&mut socket).await.map_err(map_err)?;
        auth_req.write(&mut socket).await.map_err(map_err)?;
        socket.flush().await?;

        Version::read(&mut socket).await.map_err(map_err)?;
        let resp = AuthResponse::read(&mut socket).await.map_err(map_err)?;
        if resp.method() != AuthMethod::Noauth {
            return Err(rd_interface::Error::Other("Auth failed".to_string().into()));
        }

        command_req.write(&mut socket).await.map_err(map_err)?;
        socket.flush().await?;

        let command_resp = CommandResponse::read(socket).await.map_err(map_err)?;

        Ok(command_resp)
    }
}
