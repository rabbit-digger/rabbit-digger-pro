use crate::udp::{decrypt_payload, encrypt_payload};
use bytes::{Bytes, BytesMut};
use futures::{ready, Sink, SinkExt, StreamExt};
use rd_interface::{
    async_trait, impl_async_read_write, prelude::*, Address as RDAddress, AsyncRead, AsyncWrite,
    ITcpStream, IUdpSocket, ReadBuf, Stream, TcpStream, UdpSocket, NOT_IMPLEMENTED,
};
use shadowsocks::{
    context::SharedContext,
    crypto::v1::CipherKind,
    relay::{socks5::Address as SSAddress, tcprelay::crypto_io},
    ProxyClientStream, ServerConfig,
};
use socks5_protocol::Address as S5Addr;
use std::{io, net::SocketAddr, pin::Pin, str::FromStr, task};

pub struct WrapAddress(pub RDAddress);

impl From<RDAddress> for WrapAddress {
    fn from(a: RDAddress) -> Self {
        Self(a)
    }
}

impl From<WrapAddress> for SSAddress {
    fn from(w: WrapAddress) -> Self {
        match w.0 {
            RDAddress::Domain(domain, port) => SSAddress::DomainNameAddress(domain, port),
            RDAddress::SocketAddr(s) => SSAddress::SocketAddress(s),
        }
    }
}

impl From<WrapAddress> for S5Addr {
    fn from(w: WrapAddress) -> Self {
        match w.0 {
            RDAddress::Domain(domain, port) => S5Addr::Domain(domain, port),
            RDAddress::SocketAddr(s) => S5Addr::SocketAddr(s),
        }
    }
}

#[rd_config]
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
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

impl From<Cipher> for CipherKind {
    fn from(c: Cipher) -> Self {
        let s: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
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
    socket: UdpSocket,
    method: CipherKind,
    key: Box<[u8]>,
    server_addr: RDAddress,
}

impl WrapSSUdp {
    pub fn new(socket: UdpSocket, svr_cfg: &ServerConfig, server_addr: RDAddress) -> Self {
        let key = svr_cfg.key().to_vec().into_boxed_slice();
        let method = svr_cfg.method();

        WrapSSUdp {
            socket,
            method,
            key,
            server_addr,
        }
    }
}

impl Stream for WrapSSUdp {
    type Item = io::Result<(BytesMut, SocketAddr)>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        // Waiting for response from server SERVER -> CLIENT
        let (mut recv_buf, _target_addr) = match ready!(self.socket.poll_next_unpin(cx)) {
            Some(r) => r?,
            None => return task::Poll::Ready(None),
        };
        let (n, addr) = decrypt_payload(self.method, &self.key, &mut recv_buf[..])?;

        Some(Ok((
            recv_buf.split_to(n),
            match addr {
                S5Addr::Domain(_, _) => unreachable!("Udp recv_from domain name"),
                S5Addr::SocketAddr(s) => s,
            },
        )))
        .into()
    }
}

impl Sink<(Bytes, RDAddress)> for WrapSSUdp {
    type Error = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.socket.poll_ready_unpin(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        (payload, target): (Bytes, RDAddress),
    ) -> Result<(), Self::Error> {
        let mut send_buf = BytesMut::new();
        let addr: S5Addr = WrapAddress::from(target).into();
        encrypt_payload(self.method, &self.key, &addr, &payload, &mut send_buf)?;

        let server_addr = self.server_addr.clone();
        self.socket
            .start_send_unpin((send_buf.freeze(), server_addr))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.socket.poll_flush_unpin(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.socket.poll_close_unpin(cx)
    }
}

#[async_trait]
impl IUdpSocket for WrapSSUdp {
    async fn local_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}

pub struct CryptoStream<S>(crypto_io::CryptoStream<S>, SharedContext);

impl<S> CryptoStream<S> {
    pub fn from_stream(context: SharedContext, stream: S, method: CipherKind, key: &[u8]) -> Self {
        CryptoStream(
            crypto_io::CryptoStream::<S>::from_stream(&context, stream, method, key),
            context,
        )
    }
}

impl<S> AsyncRead for CryptoStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> task::Poll<io::Result<()>> {
        let CryptoStream(s, c) = Pin::get_mut(self);
        s.poll_read_decrypted(cx, &c, buf)
    }
}

impl<S> AsyncWrite for CryptoStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> task::Poll<Result<usize, io::Error>> {
        self.0.poll_write_encrypted(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), io::Error>> {
        self.0.poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), io::Error>> {
        self.0.poll_shutdown(cx)
    }
}
