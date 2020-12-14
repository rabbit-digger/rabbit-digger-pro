use super::common::Address;
use apir::traits::{
    AsyncRead, AsyncWrite, ProxyTcpListener, ProxyTcpStream, ProxyUdpSocket, Spawn, TcpListener,
    TcpStream,
};
use futures::{
    future::try_join,
    io::{copy, Cursor},
    prelude::*,
};
use std::{
    io::{Error, ErrorKind, Result},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
};

pub enum AuthMethod {
    NoAuth,
}

struct ServerConfig<PR> {
    pr: PR,
    _auth_method: AuthMethod,
}

pub struct Socks5Server<PR, PRL> {
    config: Arc<ServerConfig<PR>>,
    prl: PRL,
}

impl<PR, PRL> Socks5Server<PR, PRL>
where
    PR: ProxyTcpStream + ProxyUdpSocket + 'static,
    PRL: ProxyTcpListener + Spawn + 'static,
{
    async fn serve_connection(
        cfg: Arc<ServerConfig<PR>>,
        mut socket: PRL::TcpStream,
    ) -> Result<()> {
        let default_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
        let ServerConfig { pr, .. } = &*cfg;

        let mut buf = [0u8; 2];
        socket.read_exact(&mut buf).await?;
        if buf[0] != 0x05 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("client request error {:x}", buf[0]),
            ));
        }

        let methods_len = buf[1] as usize;
        let mut methods = vec![0u8; methods_len];
        socket.read_exact(&mut methods).await?;

        // Find no auth
        if let Some(i) = methods.iter().position(|i| *i == 0) {
            socket.write_all(&[0x05, i as u8]).await?;
            socket.flush().await?;
        } else {
            // No acceptable methods
            socket.write_all(&[0x05, 0xFF]).await?;
            socket.flush().await?;
            return Ok(());
        }

        let mut buf = [0u8; 3];
        socket.read_exact(&mut buf).await?;
        match buf {
            // VER: 5, CMD: 1(CONNECT), RSV: 0
            [0x05, 0x01, 0x00] => {
                let dst = match Address::read(&mut socket).await?.to_socket_addr() {
                    Some(a) => a,
                    None => {
                        socket
                            .write_all(&[
                                0x05, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            ])
                            .await?;
                        socket.flush().await?;
                        return Ok(());
                    }
                };
                let out = match pr.tcp_connect(dst).await {
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
            _ => {
                return Ok(());
            }
        };

        Ok(())
    }
    pub fn new(pr: PR, prl: PRL, _auth_method: AuthMethod) -> Self {
        Self {
            config: Arc::new(ServerConfig { pr, _auth_method }),
            prl,
        }
    }
    pub async fn serve(self, port: u16) -> Result<()> {
        let listener = self
            .prl
            .tcp_bind(SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), port))
            .await?;
        self.serve_listener(listener).await
    }
    pub async fn serve_listener(self, listener: PRL::TcpListener) -> Result<()> {
        loop {
            let (socket, _) = listener.accept().await?;
            let _ = self
                .prl
                .spawn(Self::serve_connection(self.config.clone(), socket));
        }
    }
}

async fn pipe<S1, S2>(s1: S1, s2: S2) -> Result<(u64, u64)>
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
