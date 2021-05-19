use crate::udp::{decrypt_payload, encrypt_payload};
use bytes::BytesMut;
use rd_interface::{
    async_trait, impl_async_read_write,
    registry::ResolveNetRef,
    schemars::{self, JsonSchema},
    Address as RDAddress, ITcpStream, IUdpSocket, TcpStream, UdpSocket, NOT_IMPLEMENTED,
};
use serde_derive::{Deserialize, Serialize};
use shadowsocks::{
    context::SharedContext, crypto::v1::CipherKind, relay::socks5::Address as SSAddress,
    ProxyClientStream, ServerAddr, ServerConfig,
};
use std::{net::SocketAddr, str::FromStr};

pub struct WrapAddress(pub RDAddress);

impl From<RDAddress> for WrapAddress {
    fn from(a: RDAddress) -> Self {
        Self(a)
    }
}

impl Into<SSAddress> for WrapAddress {
    fn into(self) -> SSAddress {
        match self.0 {
            RDAddress::Domain(domain, port) => SSAddress::DomainNameAddress(domain, port),
            RDAddress::SocketAddr(s) => SSAddress::SocketAddress(s),
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum Cipher {
    #[serde(rename = "none")]
    NONE,
    #[serde(rename = "table")]
    SS_TABLE,
    #[serde(rename = "rc4-md5")]
    SS_RC4_MD5,
    #[serde(rename = "aes-128-ctr")]
    AES_128_CTR,
    #[serde(rename = "aes-192-ctr")]
    AES_192_CTR,
    #[serde(rename = "aes-256-ctr")]
    AES_256_CTR,
    #[serde(rename = "aes-128-cfb1")]
    AES_128_CFB1,
    #[serde(rename = "aes-128-cfb8")]
    AES_128_CFB8,
    #[serde(rename = "aes-128-cfb")]
    AES_128_CFB128,
    #[serde(rename = "aes-192-cfb1")]
    AES_192_CFB1,
    #[serde(rename = "aes-192-cfb8")]
    AES_192_CFB8,
    #[serde(rename = "aes-192-cfb")]
    AES_192_CFB128,
    #[serde(rename = "aes-256-cfb1")]
    AES_256_CFB1,
    #[serde(rename = "aes-256-cfb8")]
    AES_256_CFB8,
    #[serde(rename = "aes-256-cfb")]
    AES_256_CFB128,
    #[serde(rename = "aes-128-ofb")]
    AES_128_OFB,
    #[serde(rename = "aes-192-ofb")]
    AES_192_OFB,
    #[serde(rename = "aes-256-ofb")]
    AES_256_OFB,
    #[serde(rename = "camellia-128-ctr")]
    CAMELLIA_128_CTR,
    #[serde(rename = "camellia-192-ctr")]
    CAMELLIA_192_CTR,
    #[serde(rename = "camellia-256-ctr")]
    CAMELLIA_256_CTR,
    #[serde(rename = "camellia-128-cfb1")]
    CAMELLIA_128_CFB1,
    #[serde(rename = "camellia-128-cfb8")]
    CAMELLIA_128_CFB8,
    #[serde(rename = "camellia-128-cfb")]
    CAMELLIA_128_CFB128,
    #[serde(rename = "camellia-192-cfb1")]
    CAMELLIA_192_CFB1,
    #[serde(rename = "camellia-192-cfb8")]
    CAMELLIA_192_CFB8,
    #[serde(rename = "camellia-192-cfb")]
    CAMELLIA_192_CFB128,
    #[serde(rename = "camellia-256-cfb1")]
    CAMELLIA_256_CFB1,
    #[serde(rename = "camellia-256-cfb8")]
    CAMELLIA_256_CFB8,
    #[serde(rename = "camellia-256-cfb")]
    CAMELLIA_256_CFB128,
    #[serde(rename = "camellia-128-ofb")]
    CAMELLIA_128_OFB,
    #[serde(rename = "camellia-192-ofb")]
    CAMELLIA_192_OFB,
    #[serde(rename = "camellia-256-ofb")]
    CAMELLIA_256_OFB,
    #[serde(rename = "rc4")]
    RC4,
    #[serde(rename = "chacha20-ietf")]
    CHACHA20,
    #[serde(rename = "aes-128-gcm")]
    AES_128_GCM,
    #[serde(rename = "aes-256-gcm")]
    AES_256_GCM,
    #[serde(rename = "chacha20-ietf-poly1305")]
    CHACHA20_POLY1305,
    #[serde(rename = "aes-128-ccm")]
    AES_128_CCM,
    #[serde(rename = "aes-256-ccm")]
    AES_256_CCM,
    #[serde(rename = "aes-128-gcm-siv")]
    AES_128_GCM_SIV,
    #[serde(rename = "aes-256-gcm-siv")]
    AES_256_GCM_SIV,
    #[serde(rename = "xchacha20-ietf-poly1305")]
    XCHACHA20_POLY1305,
    #[serde(rename = "sm4-gcm")]
    SM4_GCM,
    #[serde(rename = "sm4-ccm")]
    SM4_CCM,
}

impl ResolveNetRef for Cipher {}

impl Into<CipherKind> for Cipher {
    fn into(self) -> CipherKind {
        let s: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&self).unwrap()).unwrap();
        CipherKind::from_str(s.as_str().unwrap()).unwrap()
    }
}

pub struct WrapSSTcp(pub ProxyClientStream<TcpStream>);

impl_async_read_write!(WrapSSTcp, 0);

#[async_trait]
impl ITcpStream for WrapSSTcp {
    async fn peer_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    async fn local_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}

pub struct WrapSSUdp {
    context: SharedContext,
    socket: UdpSocket,
    method: CipherKind,
    key: Box<[u8]>,
    server_addr: RDAddress,
}

impl WrapSSUdp {
    pub fn new(context: SharedContext, socket: UdpSocket, svr_cfg: &ServerConfig) -> Self {
        let key = svr_cfg.key().to_vec().into_boxed_slice();
        let method = svr_cfg.method();
        let server_addr = match svr_cfg.addr().clone() {
            ServerAddr::DomainName(d, p) => RDAddress::Domain(d, p),
            ServerAddr::SocketAddr(s) => RDAddress::SocketAddr(s),
        };

        WrapSSUdp {
            context,
            socket,
            method,
            key,
            server_addr,
        }
    }
}

#[async_trait]
impl IUdpSocket for WrapSSUdp {
    async fn local_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    async fn recv_from(&self, recv_buf: &mut [u8]) -> rd_interface::Result<(usize, SocketAddr)> {
        // Waiting for response from server SERVER -> CLIENT
        let (recv_n, target_addr) = self.socket.recv_from(recv_buf).await?;
        let (n, addr) = decrypt_payload(
            &self.context,
            self.method,
            &self.key,
            &mut recv_buf[..recv_n],
        )
        .await?;

        log::trace!(
            "UDP server client receive from {}, addr {}, packet length {} bytes, payload length {} bytes",
            target_addr,
            addr,
            recv_n,
            n,
        );

        Ok((
            n,
            match addr {
                SSAddress::DomainNameAddress(_, _) => unreachable!("Udp recv_from domain name"),
                SSAddress::SocketAddress(s) => s,
            },
        ))
    }

    async fn send_to(&self, payload: &[u8], target: RDAddress) -> rd_interface::Result<usize> {
        let mut send_buf = BytesMut::new();
        let addr: SSAddress = WrapAddress(target).into();
        encrypt_payload(
            &self.context,
            self.method,
            &self.key,
            &addr,
            payload,
            &mut send_buf,
        );

        log::trace!(
            "UDP server client send to, addr {}, payload length {} bytes, packet length {} bytes",
            addr,
            payload.len(),
            send_buf.len()
        );

        let send_len = self
            .socket
            .send_to(&send_buf, self.server_addr.clone())
            .await?;

        if send_buf.len() != send_len {
            log::warn!(
                "UDP server client send {} bytes, but actually sent {} bytes",
                send_buf.len(),
                send_len
            );
        }

        Ok(send_len)
    }
}
