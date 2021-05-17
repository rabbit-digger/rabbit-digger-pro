use ::std::{io, pin::Pin, task};
use std::{
    io::{Cursor, Write},
    net::SocketAddr,
};

use futures::ready;
use rd_interface::{
    async_trait,
    error::map_other,
    impl_async_read,
    registry::NetRef,
    schemars::{self, JsonSchema},
    Address as RdAddress, Arc, AsyncWrite, Config, INet, ITcpStream, IntoAddress, IntoDyn, Result,
    TcpListener, TcpStream, UdpSocket, NOT_ENABLED, NOT_IMPLEMENTED,
};
use serde_derive::Deserialize;
use sha2::{Digest, Sha224};
use socks5_protocol::{sync::FromIO, Address as S5Addr};
use tokio_rustls::{
    client::TlsStream,
    rustls::ClientConfig,
    webpki::{DNSName, DNSNameRef},
    TlsConnector,
};

pub struct TrojanNet {
    config: TrojanNetConfig,
    connector: TlsConnector,
    sni: DNSName,
    password: String,
}

pub struct TrojanTcp {
    stream: TlsStream<TcpStream>,
    head: Option<Vec<u8>>,
    is_first: bool,
}

impl_async_read!(TrojanTcp, stream);

impl AsyncWrite for TrojanTcp {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> task::Poll<io::Result<usize>> {
        loop {
            let Self {
                stream,
                head,
                is_first,
            } = &mut *self;
            let stream = Pin::new(stream);
            let len = match head {
                Some(head) => {
                    if *is_first {
                        head.extend(buf);
                        *is_first = false;
                    }

                    let sent = ready!(stream.poll_write(cx, &head))?;
                    head.drain(..sent);
                    head.len()
                }
                None => break,
            };
            if len == 0 {
                *head = None;
                return task::Poll::Ready(Ok(buf.len()));
            }
        }

        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[async_trait]
impl ITcpStream for TrojanTcp {
    async fn peer_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    async fn local_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}

impl TrojanNet {
    pub fn new(config: TrojanNetConfig) -> Result<Self> {
        let mut client_config = ClientConfig::default();
        client_config
            .root_store
            .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
        let connector = TlsConnector::from(Arc::new(client_config));
        let sni = DNSNameRef::try_from_ascii_str(&config.sni)
            .map_err(map_other)?
            .into();

        let password = hex::encode(Sha224::digest(config.password.as_bytes()));
        Ok(TrojanNet {
            config,
            connector,
            sni,
            password,
        })
    }
}

#[derive(Debug, Deserialize, Clone, Config, JsonSchema)]
pub struct TrojanNetConfig {
    #[serde(default)]
    net: NetRef,

    /// hostname:port
    server: String,
    password: String,
    #[serde(default)]
    udp: bool,

    sni: String,
    #[serde(default)]
    skip_cert_verify: bool,
}

#[async_trait]
impl INet for TrojanNet {
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: RdAddress,
    ) -> Result<TcpStream> {
        let addr = match addr {
            RdAddress::SocketAddr(s) => S5Addr::SocketAddr(s),
            RdAddress::Domain(d, p) => S5Addr::Domain(d, p),
        };

        let stream = self
            .config
            .net
            .tcp_connect(ctx, self.config.server.into_address()?)
            .await?;

        let stream = self.connector.connect(self.sni.as_ref(), stream).await?;

        let head = Vec::<u8>::new();
        let mut writer = Cursor::new(head);
        writer.write_all(self.password.as_bytes())?;
        writer.write_all(b"\r\n")?;
        // Connect
        writer.write_all(b"\x01")?;
        addr.write_to(&mut writer).map_err(|e| e.to_io_err())?;
        writer.write_all(b"\r\n")?;

        let tcp = TrojanTcp {
            stream,
            head: Some(writer.into_inner()),
            is_first: true,
        };
        Ok(tcp.into_dyn())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        _addr: RdAddress,
    ) -> Result<TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        _addr: RdAddress,
    ) -> Result<UdpSocket> {
        if !self.config.udp {
            return Err(NOT_ENABLED);
        }
        let cfg = self.config.clone();
        let socket = self
            .config
            .net
            .udp_bind(ctx, "0.0.0.0:0".into_address()?)
            .await?;

        todo!()
    }
}
