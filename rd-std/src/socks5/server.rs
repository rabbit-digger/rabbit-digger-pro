use super::common::{pack_udp, parse_udp, sa2ra};
use crate::util::{connect_tcp, connect_udp};
use anyhow::Context as AnyhowContext;
use futures::{ready, Sink, SinkExt, Stream, StreamExt};
use rd_interface::{
    async_trait, Address as RdAddr, Address as RDAddr, Bytes, BytesMut, Context, IServer,
    IUdpChannel, IntoDyn, Net, Result, TcpStream, UdpSocket,
};
use socks5_protocol::{
    Address, AuthMethod, AuthRequest, AuthResponse, Command, CommandReply, CommandRequest,
    CommandResponse, Version,
};
use std::{
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    pin::Pin,
    sync::Arc,
    task,
};
use tokio::io::{AsyncWriteExt, BufWriter};

struct Socks5ServerConfig {
    net: Net,
    listen_net: Net,
}

#[derive(Clone)]
pub struct Socks5Server {
    cfg: Arc<Socks5ServerConfig>,
}

impl Socks5Server {
    async fn handle_command_request(
        &self,
        mut socket: &mut BufWriter<TcpStream>,
    ) -> anyhow::Result<CommandRequest> {
        let version = Version::read(&mut socket).await?;
        let auth_req = AuthRequest::read(&mut socket).await?;

        let method = auth_req.select_from(&[AuthMethod::Noauth]);
        let auth_resp = AuthResponse::new(method);
        // TODO: do auth here

        version.write(&mut socket).await?;
        auth_resp.write(&mut socket).await?;
        socket.flush().await?;

        let cmd_req = CommandRequest::read(&mut socket).await?;

        Ok(cmd_req)
    }
    async fn response_command_error(
        &self,
        mut socket: &mut BufWriter<TcpStream>,
        e: impl std::convert::TryInto<io::Error>,
    ) -> anyhow::Result<()> {
        CommandResponse::error(e).write(&mut socket).await?;
        socket.flush().await?;
        return Ok(());
    }
    pub async fn serve_connection(self, socket: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
        let mut socket = BufWriter::with_capacity(512, socket);

        let default_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
        let Socks5ServerConfig { net, listen_net } = &*self.cfg;
        let local_ip = socket.get_ref().local_addr().await?.ip();

        let cmd_req = self
            .handle_command_request(&mut socket)
            .await
            .context("handle command request")?;

        match cmd_req.command {
            Command::Connect => {
                let dst = sa2ra(cmd_req.address);
                let ctx = &mut Context::from_socketaddr(addr);
                let out = match net.tcp_connect(ctx, &dst).await {
                    Ok(socket) => socket,
                    Err(e) => return self.response_command_error(&mut socket, e).await,
                };

                let addr = out.local_addr().await.unwrap_or(default_addr).into();
                CommandResponse::success(addr).write(&mut socket).await?;
                socket.flush().await.context("command response")?;

                let socket = socket.into_inner();

                connect_tcp(ctx, out, socket).await.context("connect tcp")?;
            }
            Command::UdpAssociate => {
                let dst = match cmd_req.address {
                    Address::SocketAddr(addr) => rd_interface::Address::any_addr_port(&addr),
                    _ => {
                        CommandResponse::reply_error(CommandReply::AddressTypeNotSupported)
                            .write(&mut socket)
                            .await?;

                        socket.flush().await?;
                        return Ok(());
                    }
                };
                let ctx = &mut Context::from_socketaddr(addr);
                let out = match net.udp_bind(ctx, &dst).await {
                    Ok(socket) => socket,
                    Err(e) => return self.response_command_error(&mut socket, e).await,
                };
                let udp = listen_net
                    .udp_bind(
                        &mut Context::from_socketaddr(addr),
                        &RdAddr::any_addr_port(&addr),
                    )
                    .await?;

                // success
                let udp_port = match udp.local_addr().await {
                    Ok(a) => a.port(),
                    Err(e) => return self.response_command_error(&mut socket, e).await,
                };
                let addr: SocketAddr = (local_ip, udp_port).into();
                let addr: Address = addr.into();

                CommandResponse::success(addr).write(&mut socket).await?;
                socket.flush().await.context("command response")?;

                let socket = socket.into_inner();

                let udp_channel = Socks5UdpSocket {
                    udp,
                    _tcp: socket,
                    endpoint: None,
                };
                connect_udp(ctx, udp_channel.into_dyn(), out)
                    .await
                    .context("connect udp")?;
            }
            _ => {
                return Ok(());
            }
        };

        Ok(())
    }
    pub fn new(listen_net: Net, net: Net) -> Self {
        Self {
            cfg: Arc::new(Socks5ServerConfig { net, listen_net }),
        }
    }
}

pub struct Socks5UdpSocket {
    udp: UdpSocket,
    // keep connection
    _tcp: TcpStream,
    endpoint: Option<SocketAddr>,
}

impl Stream for Socks5UdpSocket {
    type Item = io::Result<(Bytes, RDAddr)>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let (bytes, from_addr) = match ready!(self.udp.poll_next_unpin(cx)) {
            Some(i) => i,
            None => return None.into(),
        }?;
        if self.endpoint.is_none() {
            self.endpoint = Some(from_addr);
        }

        let (addr, payload) = parse_udp(bytes.freeze())?;

        Some(Ok((payload, addr))).into()
    }
}

impl Sink<(BytesMut, SocketAddr)> for Socks5UdpSocket {
    type Error = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.udp.poll_ready_unpin(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        (buf, saddr): (BytesMut, SocketAddr),
    ) -> Result<(), Self::Error> {
        let bytes = Bytes::copy_from_slice(&pack_udp(saddr.into(), &buf[..])?[..]);
        match self.endpoint {
            Some(endpoint) => self.udp.start_send_unpin((bytes, endpoint.into())),
            None => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "udp endpoint not set",
            )),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.udp.poll_flush_unpin(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.udp.poll_close_unpin(cx)
    }
}

impl IUdpChannel for Socks5UdpSocket {}

pub struct Socks5 {
    server: Socks5Server,
    listen_net: Net,
    bind: RdAddr,
}

#[async_trait]
impl IServer for Socks5 {
    async fn start(&self) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), &self.bind)
            .await?;

        loop {
            let (socket, addr) = listener.accept().await?;
            let server = self.server.clone();
            let _ = tokio::spawn(async move {
                if let Err(e) = server.serve_connection(socket, addr).await {
                    tracing::error!("Error when serve_connection: {:?}", e)
                }
            });
        }
    }
}

impl Socks5 {
    pub fn new(listen_net: Net, net: Net, bind: RdAddr) -> Self {
        Socks5 {
            server: Socks5Server::new(listen_net.clone(), net),
            listen_net,
            bind,
        }
    }
}
