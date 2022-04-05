use std::{
    io::{Cursor, Write},
    time::Duration,
};

use crate::{stream::IOStream, websocket::WebSocketStream};
use once_cell::sync::OnceCell;
use rd_interface::{
    async_trait, prelude::*, registry::NetRef, Address as RdAddress, Address, Error, INet, IntoDyn,
    Net, Result, TcpStream, UdpSocket,
};
use rd_std::tls::{TlsConnector, TlsConnectorConfig};
use sha2::{Digest, Sha224};
use socks5_protocol::{sync::FromIO, Address as S5Addr};
use tokio::time::timeout;

mod tcp;
mod udp;

pub struct TrojanNet {
    net: Net,
    server: RdAddress,
    connector: OnceCell<Result<TlsConnector>>,
    password: String,
    websocket: Option<WebSocket>,
    tls_config: TlsConnectorConfig,
    sni: String,
    handshake_timeout: Option<u64>,
}

impl TrojanNet {
    pub fn new(config: TrojanNetConfig) -> Result<Self> {
        let tls_config = TlsConnectorConfig {
            skip_cert_verify: config.skip_cert_verify,
        };
        let server = config.server.clone();

        let password = hex::encode(Sha224::digest(config.password.as_bytes()));
        Ok(TrojanNet {
            net: (*config.net).clone(),
            server,
            connector: OnceCell::new(),
            password,
            websocket: config.websocket,
            tls_config,
            sni: config.sni.unwrap_or_else(|| config.server.host()),
            handshake_timeout: config.handshake_timeout,
        })
    }
}

#[rd_config]
#[derive(Debug, Clone)]
pub struct WebSocket {
    host: String,
    path: String,
}

#[rd_config]
#[derive(Debug, Clone)]
pub struct TrojanNetConfig {
    #[serde(default)]
    net: NetRef,

    /// hostname:port
    server: Address,
    /// password in plain text
    password: String,

    /// sni
    #[serde(default)]
    sni: Option<String>,
    /// skip certificate verify
    #[serde(default)]
    skip_cert_verify: bool,

    /// enabled websocket support
    #[serde(default)]
    websocket: Option<WebSocket>,

    /// timeout of TLS handshake, in seconds.
    handshake_timeout: Option<u64>,
}

impl TrojanNet {
    fn get_connecter(&self) -> Result<&TlsConnector> {
        let connector = self
            .connector
            .get_or_init(|| TlsConnector::new(self.tls_config.clone()))
            .as_ref()
            .map_err(|e| Error::other(format!("Failed to create tls connector: {:?}", e)))?;
        Ok(&connector)
    }
    // cmd 1 for Connect, 3 for Udp associate
    fn make_head(&self, cmd: u8, addr: S5Addr) -> Result<Vec<u8>> {
        let head = Vec::<u8>::new();
        let mut writer = Cursor::new(head);

        writer.write_all(self.password.as_bytes())?;
        writer.write_all(b"\r\n")?;
        // Connect
        writer.write_all(&[cmd])?;
        addr.write_to(&mut writer).map_err(|e| e.to_io_err())?;
        writer.write_all(b"\r\n")?;

        Ok(writer.into_inner())
    }
    async fn connect_stream(&self, stream: impl IOStream + 'static) -> Result<Box<dyn IOStream>> {
        let stream = self.get_connecter()?.connect(&self.sni, stream).await?;

        Ok(match &self.websocket {
            Some(ws) => Box::new(WebSocketStream::connect(stream, &ws.host, &ws.path).await?),
            None => Box::new(stream),
        })
    }
    async fn get_stream(&self, ctx: &mut rd_interface::Context) -> Result<Box<dyn IOStream>> {
        let timeout_sec = self.handshake_timeout.unwrap_or(10);

        let stream = self.net.tcp_connect(ctx, &self.server).await?;
        timeout(
            Duration::from_secs(timeout_sec),
            self.connect_stream(stream),
        )
        .await?
    }
}

pub(crate) fn ra2sa(addr: RdAddress) -> S5Addr {
    match addr {
        RdAddress::SocketAddr(s) => S5Addr::SocketAddr(s),
        RdAddress::Domain(d, p) => S5Addr::Domain(d, p),
    }
}

#[async_trait]
impl INet for TrojanNet {
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &RdAddress,
    ) -> Result<TcpStream> {
        let stream = self.get_stream(ctx).await?;
        let head = self.make_head(1, ra2sa(addr.clone()))?;

        let tcp = tcp::TrojanTcp::new(stream, head);
        Ok(tcp.into_dyn())
    }

    async fn udp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &RdAddress,
    ) -> Result<UdpSocket> {
        let stream = self.get_stream(ctx).await?;
        let head = self.make_head(3, ra2sa(addr.clone()))?;

        let udp = udp::TrojanUdp::new(stream, head);

        Ok(udp.into_dyn())
    }
}
