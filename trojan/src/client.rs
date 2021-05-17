use std::io::{Cursor, Write};

use rd_interface::{
    async_trait,
    error::map_other,
    registry::NetRef,
    schemars::{self, JsonSchema},
    Address as RdAddress, Arc, Config, INet, IntoAddress, IntoDyn, Net, Result, TcpListener,
    TcpStream, UdpSocket, NOT_ENABLED, NOT_IMPLEMENTED,
};
use serde_derive::Deserialize;
use sha2::{Digest, Sha224};
use socks5_protocol::{sync::FromIO, Address as S5Addr};
use tokio_rustls::{
    rustls::ClientConfig,
    webpki::{DNSName, DNSNameRef},
    TlsConnector,
};

mod tcp;
mod udp;

pub struct TrojanNet {
    net: Net,
    server: RdAddress,
    connector: TlsConnector,
    sni: DNSName,
    password: String,
    udp: bool,
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
        let server = config.server.into_address()?;

        let password = hex::encode(Sha224::digest(config.password.as_bytes()));
        Ok(TrojanNet {
            net: config.net.net(),
            server,
            connector,
            sni,
            password,
            udp: config.udp,
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

impl TrojanNet {
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
        addr: RdAddress,
    ) -> Result<TcpStream> {
        let stream = self.net.tcp_connect(ctx, self.server.clone()).await?;
        let stream = self.connector.connect(self.sni.as_ref(), stream).await?;
        let head = self.make_head(1, ra2sa(addr))?;

        let tcp = tcp::TrojanTcp::new(stream, head);
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
        addr: RdAddress,
    ) -> Result<UdpSocket> {
        if !self.udp {
            return Err(NOT_ENABLED);
        }
        let stream = self.net.tcp_connect(ctx, self.server.clone()).await?;
        let stream = self.connector.connect(self.sni.as_ref(), stream).await?;
        let head = self.make_head(3, ra2sa(addr))?;

        let udp = udp::TrojanUdp::new(stream, head);

        Ok(udp.into_dyn())
    }
}
