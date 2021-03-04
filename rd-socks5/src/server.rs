use super::{
    auth::{auth_server, Method, NoAuth},
    common::Address,
};
use async_std::task::spawn;
use futures::{
    future::try_join,
    io::{copy, Cursor},
    prelude::*,
};
use rd_interface::{
    async_trait, AsyncRead, AsyncWrite, Context, IServer, IntoAddress, Net, Result, TcpListener,
    TcpStream,
};
use std::{
    io,
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
    async fn start(&self) -> Result<()> {
        self.serve(self.port).await
    }

    async fn stop(&self) -> Result<()> {
        // TODO
        Ok(())
    }
}

impl Socks5Server {
    async fn serve_connection(cfg: Arc<ServerConfig>, mut socket: TcpStream) -> Result<()> {
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
                let out = match net.tcp_connect(&mut Context::new(), dst).await {
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

                pipe(out, socket).await?;
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
                    .udp_bind(&mut Context::new(), "0.0.0.0:0".into_address()?)
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
    pub async fn serve(&self, port: u16) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(
                &mut Context::new(),
                SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), port).into_address()?,
            )
            .await?;
        self.serve_listener(listener).await
    }
    pub async fn serve_listener(&self, listener: TcpListener) -> Result<()> {
        loop {
            let (socket, _) = listener.accept().await?;
            let cfg = self.config.clone();
            let _ = spawn(Self::serve_connection(cfg, socket));
        }
    }
}

async fn pipe<S1, S2>(s1: S1, s2: S2) -> io::Result<(u64, u64)>
where
    S1: AsyncRead + AsyncWrite,
    S2: AsyncRead + AsyncWrite,
{
    let (mut read_1, mut write_1) = s1.split();
    let (mut read_2, mut write_2) = s2.split();

    try_join(
        async {
            let r = copy(&mut read_1, &mut write_2).await;
            write_2.close().await?;
            r
        },
        async {
            let r = copy(&mut read_2, &mut write_1).await;
            write_1.close().await?;
            r
        },
    )
    .await
}
