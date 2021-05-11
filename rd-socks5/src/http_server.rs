use rd_interface::{ConnectionPool, Net, TcpStream};
use std::net::SocketAddr;

#[derive(Clone)]
pub struct HttpServer {
    net: Net,
}

impl HttpServer {
    pub async fn serve_connection(
        self,
        socket: TcpStream,
        addr: SocketAddr,
        pool: ConnectionPool,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    pub fn new(net: Net) -> Self {
        Self { net }
    }
}
