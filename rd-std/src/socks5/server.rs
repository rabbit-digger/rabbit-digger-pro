use super::common::{pack_udp, parse_udp, Address};
use super::protocol::{
    self, AuthMethod, AuthRequest, AuthResponse, CommandRequest, CommandResponse, Version,
};
use futures::{io::BufWriter, prelude::*};
use protocol::Command;
use rd_interface::{
    async_trait, pool::IUdpChannel, util::connect_tcp, ConnectionPool, Context, IServer,
    IntoAddress, IntoDyn, Net, Result, TcpStream, UdpSocket,
};
use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4},
    sync::{Arc, RwLock},
};

struct Config {
    net: Net,
    listen_net: Net,
}

#[derive(Clone)]
pub struct Socks5Server {
    cfg: Arc<Config>,
}

impl Socks5Server {
    pub async fn serve_connection(
        self,
        socket: TcpStream,
        addr: SocketAddr,
        pool: ConnectionPool,
    ) -> anyhow::Result<()> {
        let default_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
        let Config { net, listen_net } = &*self.cfg;
        let local_ip = socket.local_addr().await?.ip();
        let (mut rx, tx) = socket.split();
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
                let dst = cmd_req.address.into();
                let out = match net
                    .tcp_connect(&mut Context::from_socketaddr(addr), dst)
                    .await
                {
                    Ok(socket) => socket,
                    Err(e) => {
                        CommandResponse::error(e).write(&mut tx).await?;
                        tx.flush().await?;
                        return Ok(());
                    }
                };

                let addr: Address = out.local_addr().await.unwrap_or(default_addr).into();
                CommandResponse::success(addr).write(&mut tx).await?;
                tx.flush().await?;

                let socket = rx.reunite(tx.into_inner()).unwrap();

                connect_tcp(out, socket).await?;
            }
            Command::UdpAssociate => {
                let dst = match cmd_req.address {
                    Address::SocketAddr(SocketAddr::V4(_)) => rd_interface::Address::SocketAddr(
                        SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0),
                    ),
                    Address::SocketAddr(SocketAddr::V6(_)) => rd_interface::Address::SocketAddr(
                        SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0),
                    ),
                    _ => {
                        CommandResponse::reply_error(
                            protocol::CommandReply::AddressTypeNotSupported,
                        )
                        .write(&mut tx)
                        .await?;

                        tx.flush().await?;
                        return Ok(());
                    }
                };
                let out = match net
                    .udp_bind(&mut Context::from_socketaddr(addr), dst.into())
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
                        "0.0.0.0:0".into_address()?,
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

                let socket = rx.reunite(tx.into_inner()).unwrap();

                let udp_channel = Socks5UdpSocket(udp, socket, RwLock::new(None));
                pool.connect_udp(udp_channel.into_dyn(), out).await?;
            }
            _ => {
                return Ok(());
            }
        };

        Ok(())
    }
    pub fn new(listen_net: Net, net: Net) -> Self {
        Self {
            cfg: Arc::new(Config { net, listen_net }),
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
        let saved_addr = { *self.2.read().unwrap() };
        if let None = saved_addr {
            *self.2.write().unwrap() = Some(from_addr);
        }
        bytes.truncate(recv_len);

        let (addr, payload) = parse_udp(&bytes).await?;
        let to_copy = payload.len().min(buf.len());
        buf[..to_copy].copy_from_slice(&payload[..to_copy]);

        Ok((to_copy, addr.into()))
    }

    async fn send_recv_from(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        let saddr: Address = addr.into();

        let bytes = pack_udp(saddr, buf).await?;

        let addr = { *self.2.read().unwrap() };
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
    bind: String,
}

#[async_trait]
impl IServer for Socks5 {
    async fn start(&self, pool: ConnectionPool) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), self.bind.into_address()?)
            .await?;

        loop {
            let (socket, addr) = listener.accept().await?;
            let server = self.server.clone();
            let pool2 = pool.clone();
            let _ = pool.spawn(async move {
                if let Err(e) = server.serve_connection(socket, addr, pool2).await {
                    log::error!("Error when serve_connection: {:?}", e)
                }
            });
        }
    }
}

impl Socks5 {
    pub fn new(listen_net: Net, net: Net, bind: String) -> Self {
        Socks5 {
            server: Socks5Server::new(listen_net.clone(), net),
            listen_net,
            bind,
        }
    }
}
