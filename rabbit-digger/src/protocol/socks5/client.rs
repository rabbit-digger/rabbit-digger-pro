use super::{
    auth::{auth_client, Method, NoAuth},
    common::Address,
};
use apir::traits::{async_trait, AsyncRead, AsyncWrite, ProxyTcpStream, ProxyUdpSocket, TcpStream};
use futures::{io::Cursor, prelude::*};
use std::{
    io::{Error, ErrorKind, Result},
    net::{Shutdown, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
};

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
