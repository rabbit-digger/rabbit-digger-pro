use super::common::{pack_udp, parse_udp, sa2ra};
use parking_lot::RwLock;
use rd_interface::{
    async_trait,
    util::{connect_tcp, connect_udp},
    Address as RdAddr, Context, IServer, IUdpChannel, IntoDyn, Net, Result, TcpStream, UdpSocket,
};
use socks5_protocol::{
    Address, AuthMethod, AuthRequest, AuthResponse, Command, CommandReply, CommandRequest,
    CommandResponse, Version,
};
use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
};
use tokio::io::{split, AsyncWriteExt, BufWriter};

struct Socks5ServerConfig {
    net: Net,
    listen_net: Net,
}

#[derive(Clone)]
pub struct Socks5Server {
    cfg: Arc<Socks5ServerConfig>,
}

impl Socks5Server {
    pub async fn serve_connection(self, socket: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
        let default_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
        let Socks5ServerConfig { net, listen_net } = &*self.cfg;
        let local_ip = socket.local_addr().await?.ip();
        let (mut rx, tx) = split(socket);
        let mut tx = BufWriter::with_capacity(512, tx);

        let version = Version::read(&mut rx).await?;
        let auth_req = AuthRequest::read(&mut rx).await?;

        let method = auth_req.select_from(&[AuthMethod::Noauth]);
        let auth_resp = AuthResponse::new(method);
        // TODO: do auth here

        version.write(&mut tx).await?;
        auth_resp.write(&mut tx).await?;
        tx.flush().await?;

        let cmd_req = CommandRequest::read(&mut rx).await?;

        match cmd_req.command {
            Command::Connect => {
                let dst = sa2ra(cmd_req.address);
                let out = match net
                    .tcp_connect(&mut Context::from_socketaddr(addr), &dst)
                    .await
                {
                    Ok(socket) => socket,
                    Err(e) => {
                        tracing::trace!("Failed to connect {:?}", e);
                        CommandResponse::error(e).write(&mut tx).await?;
                        tx.flush().await?;
                        return Ok(());
                    }
                };

                let addr = out.local_addr().await.unwrap_or(default_addr).into();
                CommandResponse::success(addr).write(&mut tx).await?;
                tx.flush().await?;

                let socket = rx.unsplit(tx.into_inner());

                connect_tcp(out, socket).await?;
            }
            Command::UdpAssociate => {
                let dst = match cmd_req.address {
                    Address::SocketAddr(addr) => rd_interface::Address::any_addr_port(&addr),
                    _ => {
                        CommandResponse::reply_error(CommandReply::AddressTypeNotSupported)
                            .write(&mut tx)
                            .await?;

                        tx.flush().await?;
                        return Ok(());
                    }
                };
                let out = match net
                    .udp_bind(&mut Context::from_socketaddr(addr), &dst)
                    .await
                {
                    Ok(socket) => socket,
                    Err(e) => {
                        CommandResponse::error(e).write(&mut tx).await?;
                        tx.flush().await?;
                        return Ok(());
                    }
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
                    Err(e) => {
                        CommandResponse::error(e).write(&mut tx).await?;
                        tx.flush().await?;
                        return Ok(());
                    }
                };
                let addr: SocketAddr = (local_ip, udp_port).into();
                let addr: Address = addr.into();

                CommandResponse::success(addr).write(&mut tx).await?;
                tx.flush().await?;

                let socket = rx.unsplit(tx.into_inner());

                let udp_channel = Socks5UdpSocket(udp, socket, RwLock::new(None));
                connect_udp(udp_channel.into_dyn(), out).await?;
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

pub struct Socks5UdpSocket(UdpSocket, TcpStream, RwLock<Option<SocketAddr>>);

#[async_trait]
impl IUdpChannel for Socks5UdpSocket {
    async fn recv_send_to(&self, buf: &mut [u8]) -> Result<(usize, rd_interface::Address)> {
        // 259 is max size of address, atype 1 + domain len 1 + domain 255 + port 2
        let bytes_size = 259 + buf.len();
        let mut bytes = vec![0u8; bytes_size];
        let (recv_len, from_addr) = self.0.recv_from(&mut bytes).await?;
        let saved_addr = { *self.2.read() };
        if saved_addr.is_none() {
            *self.2.write() = Some(from_addr);
        }
        bytes.truncate(recv_len);

        let (addr, payload) = parse_udp(&bytes).await?;
        let to_copy = payload.len().min(buf.len());
        buf[..to_copy].copy_from_slice(&payload[..to_copy]);

        Ok((to_copy, sa2ra(addr)))
    }

    async fn send_recv_from(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        let saddr: Address = addr.into();

        let bytes = pack_udp(saddr, buf).await?;

        let addr = { *self.2.read() };
        Ok(if let Some(addr) = addr {
            self.0.send_to(&bytes, addr.into()).await?
        } else {
            0
        })
    }
}

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
