use std::net::SocketAddr;

use hyper::{client::conn as client_conn, Body, Error, Request};

use rd_interface::{
    async_trait, impl_async_read_write, INet, ITcpStream, IntoAddress, IntoDyn, Net, Result,
    TcpStream, NOT_IMPLEMENTED,
};

pub fn map_err(e: Error) -> rd_interface::Error {
    match e {
        e => rd_interface::Error::Other(e.into()),
    }
}

pub struct HttpClient {
    address: String,
    port: u16,
    net: Net,
}

pub struct HttpTcpStream(TcpStream);

impl_async_read_write!(HttpTcpStream, 0);

#[async_trait]
impl ITcpStream for HttpTcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}

#[async_trait]
impl INet for HttpClient {
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: rd_interface::Address,
    ) -> Result<TcpStream> {
        let socket = self.net.tcp_connect(ctx, self.server()?).await?;
        let (mut request_sender, connection) =
            client_conn::handshake(socket).await.map_err(map_err)?;
        let connect_req = Request::builder()
            .method("CONNECT")
            .uri(addr.to_string())
            .body(Body::empty())
            .unwrap();
        let connection = connection.without_shutdown();
        let _connect_resp = request_sender.send_request(connect_req);
        let io = connection.await.map_err(map_err)?.io;
        let _connect_resp = _connect_resp.await.map_err(map_err)?;
        Ok(HttpTcpStream(io).into_dyn())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        _addr: rd_interface::Address,
    ) -> Result<rd_interface::TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        _addr: rd_interface::Address,
    ) -> Result<rd_interface::UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
}

impl HttpClient {
    pub fn new(net: Net, address: String, port: u16) -> Self {
        Self { address, port, net }
    }
    fn server(&self) -> Result<rd_interface::Address> {
        (self.address.as_str(), self.port)
            .into_address()
            .map_err(Into::into)
    }
}
