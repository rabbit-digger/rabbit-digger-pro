use hyper::{
    client::conn as client_conn, http, server::conn as server_conn, service::service_fn,
    upgrade::Upgraded, Body, Method, Request, Response,
};
use rd_interface::{async_trait, Address, Context, IServer, IntoAddress, Net, Result, TcpStream};
use std::net::SocketAddr;
use tracing::instrument;

#[derive(Clone)]
pub struct HttpServer {
    net: Net,
}

impl HttpServer {
    #[instrument(err, skip(self, socket))]
    pub async fn serve_connection(self, socket: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
        let net = self.net.clone();

        server_conn::Http::new()
            .http1_preserve_header_case(true)
            .http1_title_case_headers(true)
            .http1_keep_alive(true)
            .serve_connection(socket, service_fn(move |req| proxy(net.clone(), req, addr)))
            .with_upgrades()
            .await?;

        Ok(())
    }
    pub fn new(net: Net) -> Self {
        Self { net }
    }
}

pub struct Http {
    server: HttpServer,
    listen_net: Net,
    bind: Address,
}

#[async_trait]
impl IServer for Http {
    async fn start(&self) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), &self.bind)
            .await?;

        loop {
            let (socket, addr) = listener.accept().await?;
            let server = self.server.clone();
            tokio::spawn(async move {
                if let Err(e) = server.serve_connection(socket, addr).await {
                    tracing::error!("Error when serve_connection: {:?}", e);
                }
            });
        }
    }
}

impl Http {
    pub fn new(listen_net: Net, net: Net, bind: Address) -> Self {
        Http {
            server: HttpServer::new(net),
            listen_net,
            bind,
        }
    }
}

async fn proxy(net: Net, req: Request<Body>, addr: SocketAddr) -> anyhow::Result<Response<Body>> {
    if let Some(mut dst) = host_addr(req.uri()) {
        if !dst.contains(':') {
            dst += ":80"
        }
        let dst = dst.into_address()?;

        if req.method() == Method::CONNECT {
            tokio::spawn(async move {
                match hyper::upgrade::on(req).await {
                    Ok(upgraded) => {
                        let stream = net
                            .tcp_connect(&mut Context::from_socketaddr(addr), &dst)
                            .await?;
                        if let Err(e) = tunnel(stream, upgraded).await {
                            tracing::debug!("tunnel io error: {}", e);
                        };
                    }
                    Err(e) => tracing::debug!("upgrade error: {}", e),
                }
                Ok(()) as anyhow::Result<()>
            });

            Ok(Response::new(Body::empty()))
        } else {
            let stream = net
                .tcp_connect(&mut Context::from_socketaddr(addr), &dst)
                .await?;

            let (mut request_sender, connection) = client_conn::Builder::new()
                .http1_preserve_header_case(true)
                .http1_title_case_headers(true)
                .handshake(stream)
                .await?;

            tokio::spawn(connection);

            let resp = request_sender.send_request(req).await?;

            Ok(resp)
        }
    } else {
        tracing::error!("host is not socket addr: {:?}", req.uri());
        let mut resp = Response::new(Body::from("CONNECT must be to a socket address"));
        *resp.status_mut() = http::StatusCode::BAD_REQUEST;

        Ok(resp)
    }
}

fn host_addr(uri: &http::Uri) -> Option<String> {
    uri.authority().map(|auth| auth.to_string())
}

async fn tunnel(mut stream: TcpStream, mut upgraded: Upgraded) -> std::io::Result<()> {
    tokio::io::copy_bidirectional(&mut upgraded, &mut stream).await?;
    Ok(())
}
