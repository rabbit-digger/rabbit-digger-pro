use std::net::SocketAddr;

use hyper::{client::conn as client_conn, Body, Error, Request};

use rd_interface::{
    async_trait, impl_async_read_write, Address, Fd, INet, ITcpStream, IntoDyn, Net, Result,
    TcpStream, NOT_IMPLEMENTED,
};

fn map_err(e: Error) -> rd_interface::Error {
    rd_interface::Error::Other(e.into())
}

pub struct HttpClient {
    server: Address,
    net: Net,
}

pub struct HttpTcpStream(TcpStream);

#[async_trait]
impl ITcpStream for HttpTcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    fn read_passthrough(&self) -> Option<Fd> {
        self.0.read_passthrough()
    }
    fn write_passthrough(&self) -> Option<Fd> {
        self.0.write_passthrough()
    }

    impl_async_read_write!(0);
}

#[async_trait]
impl INet for HttpClient {
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &rd_interface::Address,
    ) -> Result<TcpStream> {
        let socket = self.net.tcp_connect(ctx, &self.server).await?;
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
}

impl HttpClient {
    pub fn new(net: Net, server: Address) -> Self {
        Self { server, net }
    }
}
