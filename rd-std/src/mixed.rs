use std::net::SocketAddr;

use rd_interface::{
    async_trait, prelude::*, registry::ServerFactory, Address, Context, IServer, IntoDyn, Net,
    Registry, Result, TcpStream,
};

use crate::{http::HttpServer, socks5::Socks5Server, util::PeekableTcpStream};

#[derive(Clone)]
struct HttpSocks5Server {
    http_server: HttpServer,
    socks5_server: Socks5Server,
}

impl HttpSocks5Server {
    fn new(listen_net: Net, net: Net) -> Self {
        Self {
            http_server: HttpServer::new(net.clone()),
            socks5_server: Socks5Server::new(listen_net.clone(), net.clone()),
        }
    }
    pub async fn serve_connection(self, socket: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
        let buf = &mut [0u8; 1];
        let mut socket = PeekableTcpStream::new(socket);
        socket.peek_exact(buf).await?;
        let socket = socket.into_dyn();

        match buf[0] {
            b'\x05' => self.socks5_server.serve_connection(socket, addr).await,
            _ => self.http_server.serve_connection(socket, addr).await,
        }
    }
}

pub struct HttpSocks5 {
    listen_net: Net,
    bind: Address,

    server: HttpSocks5Server,
}

#[async_trait]
impl IServer for HttpSocks5 {
    async fn start(&self) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), &self.bind)
            .await?;

        loop {
            let (socket, addr) = listener.accept().await?;

            let server = self.server.clone();
            let _ = tokio::spawn(async move {
                if let Err(e) = server.serve_connection(socket, addr).await {
                    tracing::error!("Error when serve_connection: {:?}", e)
                }
            });
        }
    }
}

impl HttpSocks5 {
    fn new(listen_net: Net, net: Net, bind: Address) -> Self {
        HttpSocks5 {
            server: HttpSocks5Server::new(listen_net.clone(), net),
            listen_net,
            bind,
        }
    }
}

#[rd_config]
#[derive(Debug)]
pub struct MixedServerConfig {
    bind: Address,
}

impl ServerFactory for HttpSocks5 {
    const NAME: &'static str = "http+socks5";
    type Config = MixedServerConfig;
    type Server = Self;

    fn new(listen: Net, net: Net, Self::Config { bind }: Self::Config) -> Result<Self> {
        Ok(HttpSocks5::new(listen, net, bind))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_server::<HttpSocks5>();
    Ok(())
}
