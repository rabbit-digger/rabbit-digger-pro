use std::io::{Cursor, Write};

use crate::tls::{TlsConnector, TlsConnectorConfig};
use rd_interface::{
    async_trait, prelude::*, registry::NetRef, Address as RdAddress, Address, INet, IntoDyn, Net,
    Result, TcpListener, TcpStream, UdpSocket, NOT_ENABLED, NOT_IMPLEMENTED,
};
use sha2::{Digest, Sha224};
use socks5_protocol::{sync::FromIO, Address as S5Addr};

mod tcp;
mod udp;

pub struct TrojanNet {
    net: Net,
    server: RdAddress,
    connector: TlsConnector,
    password: String,
    udp: bool,
}

impl TrojanNet {
    pub fn new(config: TrojanNetConfig) -> Result<Self> {
        let connector = TlsConnector::new(TlsConnectorConfig {
            skip_cert_verify: config.skip_cert_verify,
            sni: config.sni,
        })?;
        let server = config.server.clone();

        let password = hex::encode(Sha224::digest(config.password.as_bytes()));
        Ok(TrojanNet {
            net: config.net.net(),
            server,
            connector,
            password,
            udp: config.udp,
        })
    }
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

    /// enable udp or not
    #[serde(default)]
    udp: bool,

    /// sni
    sni: String,
    /// skip certificate verify
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
        let stream = self.connector.connect(stream).await?;
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
        let stream = self.connector.connect(stream).await?;
        let head = self.make_head(3, ra2sa(addr))?;

        let udp = udp::TrojanUdp::new(stream, head);

        Ok(udp.into_dyn())
    }
}
