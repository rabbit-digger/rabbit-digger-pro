use super::{
    auth::{auth_client, Method, NoAuth},
    common::Address,
};
use apir::{traits::{async_trait, AsyncRead, AsyncWrite, ProxyTcpStream, ProxyUdpSocket, TcpStream, UdpSocket}};
use futures::{io::Cursor, prelude::*};
use std::{io::{Error, ErrorKind, Result}, net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr}, ops::Add, pin::Pin, task::{Context, Poll}};

pub struct Socks5Client<PR: ProxyTcpStream> {
    server: SocketAddr,
    methods: Vec<Box<dyn Method + Send + Sync>>,
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


pub struct Socks5UdpSocket<PR: ProxyUdpSocket>(PR::UdpSocket, SocketAddr);

#[async_trait]
impl<PR> UdpSocket for Socks5UdpSocket<PR>
where
    PR: ProxyUdpSocket + ProxyTcpStream,
{
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        // 259 is max size of address, atype 1 + domain len 1 + domain 255 + port 2
        let bytes_size = 259 + buf.len();
        let mut bytes = vec![0u8; bytes_size];
        let recv_len;

        loop {
            let (len, addr) = self.0.recv_from(&mut bytes).await.unwrap();
            if addr == self.1 {
                recv_len = len;
                break;
            }
        }

        let mut cursor = Cursor::new(bytes);
        let mut header = [0u8; 3];
        cursor.read_exact(&mut header).await?;
        let addr = match header[0..3] {
            [0x00, 0x00, 0x00] => Address::read(&mut cursor).await?,
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "server response wrong RSV {} RSV {} FRAG {}",
                        header[0], header[1], header[2]
                    ),
                ))
            }
        };
        let header_len = cursor.position() as usize;
        cursor.read_exact(buf).await?;

        Ok((recv_len - header_len, addr.to_socket_addr().unwrap()))
    }

    async fn send_to(&self, _buf: &[u8], _addr: SocketAddr) -> Result<usize> {
        todo!()
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().await
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
    type UdpSocket = Socks5UdpSocket<PR>;

    async fn udp_bind(&self, addr: SocketAddr) -> Result<Self::UdpSocket> {
        let client = self.pr.udp_bind(addr).await.unwrap();
        let mut socket = self.pr.tcp_connect(self.server).await?;

        auth_client(&mut socket, &self.methods()).await?;

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);
        let mut buf = Cursor::new(Vec::new());
        buf.write_all(&[0x05u8, 0x03u8, 0x00u8]).await?;
        let addr: Address = addr.into();
        addr.write(&mut buf).await?;
        socket.write_all(&buf.into_inner()).await?;
        socket.flush().await?;

        // server reply. VER, REP, RSV
        let mut buf = [0u8; 3];
        socket.read_exact(&mut buf).await?;
        // TODO: set address to socket
        let addr = match buf[0..3] {
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

        let addr = addr.to_socket_addr().unwrap();

        Ok(Socks5UdpSocket(client, addr))
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

        auth_client(&mut socket, &self.methods()).await?;

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
        Self {
            server,
            pr,
            methods: vec![Box::new(NoAuth)],
        }
    }
    fn methods(&self) -> Vec<&(dyn Method + Send + Sync)> {
        self.methods.iter().map(|i| &**i).collect::<Vec<_>>()
    }
}
