use super::{
    auth::{auth_server, Method, NoAuth},
    common::Address,
};
use futures::{io::Cursor, prelude::*};
use rd_interface::{
    async_trait, context::common_field::SourceAddress, util::connect_tcp, ConnectionPool, Context,
    IServer, IntoAddress, Net, Result, TcpListener, TcpStream,
};
use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
};

struct ServerConfig {
    net: Net,
    methods: Vec<Box<dyn Method + Send + Sync>>,
}

pub struct Socks5Server {
    config: Arc<ServerConfig>,
    listen_net: Net,
    port: u16,
}

#[async_trait]
impl IServer for Socks5Server {
    async fn start(&self, pool: ConnectionPool) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(
                &mut Context::new(),
                SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), self.port).into_address()?,
            )
            .await?;
        self.serve_listener(pool, listener).await
    }
}

async fn new_context(addr: SocketAddr) -> Context {
    let mut ctx = Context::new();
    let _ = ctx
        .insert_common::<SourceAddress>(SourceAddress { addr })
        .await
        .ok();
    ctx
}

impl Socks5Server {
    async fn serve_connection(
        cfg: Arc<ServerConfig>,
        mut socket: TcpStream,
        addr: SocketAddr,
    ) -> Result<()> {
        let default_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
        let ServerConfig { net, methods, .. } = &*cfg;

        auth_server(
            &mut socket,
            &methods.iter().map(|i| &**i).collect::<Vec<_>>(),
        )
        .await?;

        let mut buf = [0u8; 3];
        socket.read_exact(&mut buf).await?;
        match buf {
            // VER: 5, CMD: 1(CONNECT), RSV: 0
            [0x05, 0x01, 0x00] => {
                let dst = Address::read(&mut socket).await?.into();
                let out = match net.tcp_connect(&mut new_context(addr).await, dst).await {
                    Ok(socket) => socket,
                    Err(_e) => {
                        // TODO better error
                        socket
                            .write_all(&[
                                0x05, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            ])
                            .await?;
                        socket.flush().await?;
                        return Ok(());
                    }
                };

                // success
                let mut writer = Cursor::new(Vec::new());
                writer.write_all(&[0x05, 0x00, 0x00]).await?;
                let addr: Address = out.local_addr().await.unwrap_or(default_addr).into();
                addr.write(&mut writer).await?;

                socket.write_all(&writer.into_inner()).await?;

                connect_tcp(out, socket).await?;
            }
            // UDP
            [0x05, 0x03, 0x00] => {
                let _addr = match Address::read(&mut socket).await?.to_socket_addr() {
                    Ok(a) => a,
                    Err(_) => {
                        socket
                            .write_all(&[
                                0x05, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            ])
                            .await?;
                        socket.flush().await?;
                        return Ok(());
                    }
                };
                let udp = net
                    .udp_bind(&mut new_context(addr).await, "0.0.0.0:0".into_address()?)
                    .await?;

                // success
                let mut writer = Cursor::new(Vec::new());
                writer.write_all(&[0x05, 0x00, 0x00]).await?;
                let addr: Address = udp.local_addr().await.unwrap_or(default_addr).into();
                addr.write(&mut writer).await?;

                socket.write_all(&writer.into_inner()).await?;
            }
            _ => {
                return Ok(());
            }
        };

        Ok(())
    }
    pub fn new(listen_net: Net, net: Net, port: u16) -> Self {
        Self {
            config: Arc::new(ServerConfig {
                net,
                methods: vec![Box::new(NoAuth)],
            }),
            listen_net,
            port,
        }
    }
    pub async fn serve_listener(&self, pool: ConnectionPool, listener: TcpListener) -> Result<()> {
        loop {
            let (socket, addr) = listener.accept().await?;
            let cfg = self.config.clone();
            let _ = pool.spawn(async move {
                if let Err(e) = Self::serve_connection(cfg, socket, addr).await {
                    log::error!("Error when serve_connection: {:?}", e)
                }
            });
        }
    }
}
