use apir::traits::{
    async_trait, AsyncRead, AsyncWrite, ProxyTcpListener, ProxyTcpStream, ProxyUdpSocket, Spawn,
    TcpListener, TcpStream,
};
use futures::{
    future::try_join,
    io::{copy, Cursor},
    prelude::*,
};
use std::{
    io::{Error, ErrorKind, Result},
    net::{Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, SocketAddrV4, SocketAddrV6},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub enum Address {
    IPv4(SocketAddrV4),
    IPv6(SocketAddrV6),
    Domain(String),
}

impl From<SocketAddr> for Address {
    fn from(addr: SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(v4) => Address::IPv4(v4),
            SocketAddr::V6(v6) => Address::IPv6(v6),
        }
    }
}

impl Address {
    fn to_socket_addr(self) -> Option<SocketAddr> {
        match self {
            Address::IPv4(v4) => Some(SocketAddr::V4(v4)),
            Address::IPv6(v6) => Some(SocketAddr::V6(v6)),
            _ => None,
        }
    }
    async fn read_port<R>(mut reader: R) -> Result<u16>
    where
        R: AsyncRead + Unpin,
    {
        let mut port = [0u8; 2];
        reader.read_exact(&mut port).await?;
        Ok((port[0] as u16) << 8 | port[1] as u16)
    }
    async fn write_port<W>(mut writer: W, port: u16) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        writer.write_all(&[(port >> 8) as u8, port as u8]).await
    }
    async fn write<W>(&self, mut writer: W) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        match self {
            Address::IPv4(ip) => {
                writer.write_all(&[0x01]).await?;
                writer.write_all(&ip.ip().octets()).await?;
                Self::write_port(writer, ip.port()).await?;
            }
            Address::IPv6(ip) => {
                writer.write_all(&[0x04]).await?;
                writer.write_all(&ip.ip().octets()).await?;
                Self::write_port(writer, ip.port()).await?;
            }
            Address::Domain(domain) => {
                if domain.len() >= 256 {
                    return Err(ErrorKind::InvalidInput.into());
                }
                let header = [0x03, domain.len() as u8];
                writer.write_all(&header).await?;
                writer.write_all(domain.as_bytes()).await?;
            }
        };
        Ok(())
    }
    async fn read<R>(mut reader: R) -> Result<Self>
    where
        R: AsyncRead + Unpin,
    {
        let mut atyp = [0u8; 1];
        reader.read_exact(&mut atyp).await?;

        Ok(match atyp[0] {
            1 => {
                let mut ip = [0u8; 4];
                reader.read_exact(&mut ip).await?;
                Address::IPv4(SocketAddrV4::new(
                    ip.into(),
                    Self::read_port(&mut reader).await?,
                ))
            }
            3 => {
                let mut len = [0u8; 1];
                reader.read_exact(&mut len).await?;
                let len = len[0] as usize;
                let mut domain = Vec::new();
                domain.resize(len, 0);
                reader.read_exact(&mut domain).await?;

                let domain = String::from_utf8(domain).map_err(|e| {
                    Error::new(
                        ErrorKind::InvalidData,
                        format!("bad domain {:?}", e.as_bytes()),
                    )
                })?;

                Address::Domain(domain)
            }
            4 => {
                let mut ip = [0u8; 16];
                reader.read_exact(&mut ip).await?;
                Address::IPv6(SocketAddrV6::new(
                    ip.into(),
                    Self::read_port(&mut reader).await?,
                    0,
                    0,
                ))
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("bad atyp {}", atyp[0]),
                ))
            }
        })
    }
}

// VER: 5, Method: 1, Methods: [NO_AUTH]
const AUTH_REQUEST: &[u8] = &[0x05, 0x01, 0x00];

pub struct Socks5Client<PR: ProxyTcpStream> {
    server: SocketAddr,
    pr: PR,
}

pub struct Socks5TcpStream<PR: ProxyTcpStream>(PR::TcpStream);

impl<PR: ProxyTcpStream> AsyncRead for Socks5TcpStream<PR> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}
impl<PR: ProxyTcpStream> AsyncWrite for Socks5TcpStream<PR> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

#[async_trait]
impl<PR> TcpStream for Socks5TcpStream<PR>
where
    PR: ProxyTcpStream,
{
    async fn peer_addr(&self) -> Result<SocketAddr> {
        todo!()
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        todo!()
    }

    async fn shutdown(&self, how: Shutdown) -> std::io::Result<()> {
        self.0.shutdown(how).await
    }
}

#[async_trait]
impl<PR> ProxyUdpSocket for Socks5Client<PR>
where
    PR: ProxyTcpStream + ProxyUdpSocket,
{
    type UdpSocket = PR::UdpSocket;

    async fn udp_bind(&self, _addr: SocketAddr) -> Result<Self::UdpSocket> {
        todo!()
    }
}

#[async_trait]
impl<PR> ProxyTcpStream for Socks5Client<PR>
where
    PR: ProxyTcpStream,
{
    type TcpStream = Socks5TcpStream<PR>;

    async fn tcp_connect(&self, addr: SocketAddr) -> Result<Self::TcpStream> {
        let mut socket = self.pr.tcp_connect(self.server).await?;
        socket.write_all(AUTH_REQUEST).await?;
        socket.flush().await?;

        let mut buf = [0u8; 2];
        socket.read_exact(&mut buf).await?;
        let method = match buf {
            [0x05, 0xFF] => return Err(Error::new(ErrorKind::Other, "server needs authorization")),
            [0x05, method] => method,
            _ => return Err(Error::new(ErrorKind::Other, "server refused to connect")),
        };

        match method {
            0 => {}
            _ => return Err(Error::new(ErrorKind::Other, "auth method not implement")),
        }

        // connect
        // VER: 5, CMD: 1(connect)
        let mut buf = Cursor::new(Vec::new());
        buf.write_all(&[0x05u8, 0x01, 0x00]).await?;
        let addr: Address = addr.into();
        addr.write(&mut buf).await?;
        socket.write_all(&buf.into_inner()).await?;
        socket.flush().await?;

        // server reply. VER, REP, RSV
        let mut buf = [0u8; 3];
        socket.read_exact(&mut buf).await?;
        // TODO: set address to socket
        let _addr = match buf[0..3] {
            [0x05, 0x00, 0x00] => Address::read(&mut socket).await?,
            [0x05, err] => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("server response error {}", err),
                ))
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "server response wrong VER {} REP {} RSV {}",
                        buf[0], buf[1], buf[2]
                    ),
                ))
            }
        };

        Ok(Socks5TcpStream(socket))
    }
}

impl<PR> Socks5Client<PR>
where
    PR: ProxyTcpStream,
{
    pub fn new(pr: PR, server: SocketAddr) -> Self {
        Self { server, pr }
    }
}

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
