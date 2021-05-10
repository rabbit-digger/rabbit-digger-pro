use rd_interface::{async_trait, ConnectionPool, Context, IServer, IntoAddress, Net, Result};

use super::{http_server::HttpServer, socks5_server::Socks5Server};

pub struct Socks5 {
    server: Socks5Server,
    listen_net: Net,
    bind: String,
}

#[async_trait]
impl IServer for Socks5 {
    async fn start(&self, pool: ConnectionPool) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), self.bind.into_address()?)
            .await?;

        loop {
            let (socket, addr) = listener.accept().await?;
            let server = self.server.clone();
            let pool2 = pool.clone();
            let _ = pool.spawn(async move {
                if let Err(e) = server.serve_connection(socket, addr, pool2).await {
                    log::error!("Error when serve_connection: {:?}", e)
                }
            });
        }
    }
}

impl Socks5 {
    pub fn new(listen_net: Net, net: Net, bind: String) -> Self {
        Socks5 {
            server: Socks5Server::new(listen_net.clone(), net),
            listen_net,
            bind,
        }
    }
}

pub struct Http {
    server: HttpServer,
    listen_net: Net,
    bind: String,
}

#[async_trait]
impl IServer for Http {
    async fn start(&self, pool: ConnectionPool) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), self.bind.into_address()?)
            .await?;

        loop {
            let (socket, addr) = listener.accept().await?;
            let server = self.server.clone();
            let pool2 = pool.clone();
            let _ = pool.spawn(async move {
                if let Err(e) = server.serve_connection(socket, addr, pool2).await {
                    log::error!("Error when serve_connection: {:?}", e)
                }
            });
        }
    }
}

impl Http {
    pub fn new(listen_net: Net, net: Net, bind: String) -> Self {
        Http {
            server: HttpServer::new(net),
            listen_net,
            bind,
        }
    }
}
