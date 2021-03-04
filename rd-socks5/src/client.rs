use super::{
    auth::{auth_client, Method, NoAuth},
    common::Address,
};
use futures::{io::Cursor, prelude::*};
use rd_interface::{
    async_trait, AsyncRead, AsyncWrite, INet, ITcpStream, IUdpSocket, IntoAddress, Net, Result,
    TcpStream, UdpSocket, NOT_IMPLEMENTED,
};
use std::{
    io::{self, Error, ErrorKind},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
};

pub struct Socks5Client {
    address: String,
    port: u16,
    methods: Vec<Box<dyn Method + Send + Sync>>,
    pr: Net,
}

pub struct Socks5TcpStream(TcpStream);

impl AsyncRead for Socks5TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}
impl AsyncWrite for Socks5TcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

pub struct Socks5UdpSocket(UdpSocket, TcpStream, SocketAddr);

#[async_trait]
impl IUdpSocket for Socks5UdpSocket {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        // 259 is max size of address, atype 1 + domain len 1 + domain 255 + port 2
        let bytes_size = 259 + buf.len();
        let mut bytes = vec![0u8; bytes_size];
        let recv_len = loop {
            let (len, addr) = self.0.recv_from(&mut bytes).await?;
            if addr == self.2 {
                break len;
            }
        };

        let mut cursor = Cursor::new(bytes);
        let mut header = [0u8; 3];
        cursor.read_exact(&mut header).await?;
        let addr = match header[0..3] {
            // TODO: support fragment sequence or at least give another error
            [0x00, 0x00, 0x00] => Address::read(&mut cursor).await?,
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "server response wrong RSV {} RSV {} FRAG {}",
                        header[0], header[1], header[2]
                    ),
                )
                .into())
            }
        };
        let body_len = recv_len - cursor.position() as usize;
        let to_copy = body_len.min(buf.len());
        cursor.read_exact(&mut buf[..to_copy]).await?;

        Ok((to_copy, addr.to_socket_addr()?))
    }

    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        let addr: Address = addr.into();
        let mut cursor = Cursor::new(Vec::new());
        cursor.write_all(&[0x00, 0x00, 0x00]).await?;
        addr.write(&mut cursor).await?;
        cursor.write_all(buf).await?;

        let bytes = cursor.into_inner();

        self.0.send_to(&bytes, self.2).await
    }

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
    async fn udp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: rd_interface::Address,
    ) -> Result<UdpSocket> {
        let client = self.pr.udp_bind(ctx, addr).await?;
        let mut socket = self.pr.tcp_connect(ctx, self.server()?).await?;

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
                )
                .into())
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "server response wrong VER {} REP {} RSV {}",
                        buf[0], buf[1], buf[2]
                    ),
                )
                .into())
            }
        };

        let addr = addr.to_socket_addr()?;

        Ok(Box::new(Socks5UdpSocket(client, socket, addr)))
    }
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: rd_interface::Address,
    ) -> Result<TcpStream> {
        let mut socket = self.pr.tcp_connect(ctx, self.server()?).await?;

        auth_client(&mut socket, &self.methods()).await?;

        // connect
        // VER: 5, CMD: 1(connect)
        let mut buf = Cursor::new(Vec::new());
        buf.write_all(&[0x05u8, 0x01, 0x00]).await?;
        let addr: Address = addr.into_address()?.into();
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
                )
                .into())
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "server response wrong VER {} REP {} RSV {}",
                        buf[0], buf[1], buf[2]
                    ),
                )
                .into())
            }
        };

        Ok(Box::new(Socks5TcpStream(socket)))
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        _addr: rd_interface::Address,
    ) -> Result<rd_interface::TcpListener> {
        Err(rd_interface::Error::NotImplemented)
    }
}

impl Socks5Client {
    pub fn new(pr: Net, address: String, port: u16) -> Self {
        Self {
            address,
            port,
            pr,
            methods: vec![Box::new(NoAuth)],
        }
    }
    fn methods(&self) -> Vec<&(dyn Method + Send + Sync)> {
        self.methods.iter().map(|i| &**i).collect::<Vec<_>>()
    }
    fn server(&self) -> Result<rd_interface::Address> {
        (self.address.as_str(), self.port)
            .into_address()
            .map_err(Into::into)
    }
}
