use crate::udp::{decrypt_payload, encrypt_payload};
use bytes::BytesMut;
use pin::Pin;
use rd_interface::{
    async_trait, Address as RDAddress, AsyncRead, AsyncWrite, ITcpStream, IUdpSocket, TcpStream,
    UdpSocket, NOT_IMPLEMENTED,
};
use serde::{
    de::{self, Visitor},
    Deserializer,
};
use shadowsocks::{
    context::SharedContext, crypto::v1::CipherKind, relay::socks5::Address as SSAddress,
    ProxyClientStream, ServerAddr, ServerConfig,
};
use std::{io, net::SocketAddr, pin, str::FromStr, task};
use task::{Context, Poll};
use tokio_util::compat::Compat;

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

#[derive(Debug, Clone)]
pub struct WrapCipher(CipherKind);

impl Into<CipherKind> for WrapCipher {
    fn into(self) -> CipherKind {
        self.0
    }
}

pub fn deserialize_cipher<'de, D>(de: D) -> Result<WrapCipher, D::Error>
where
    D: Deserializer<'de>,
{
    struct StrVisitor<'a>(&'a std::marker::PhantomData<()>);
    impl<'a, 'de> Visitor<'de> for StrVisitor<'a> {
        type Value = WrapCipher;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "a chiper from aes-128-gcm aes-256-gcm
aes-128-cfb aes-192-cfb aes-256-cfb
aes-128-ctr aes-192-ctr aes-256-ctr
rc4-md5 chacha20-ietf
chacha20-ietf-poly1305 xchacha20-ietf-poly1305"
            )
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match CipherKind::from_str(s) {
                Ok(c) => Ok(WrapCipher(c)),
                Err(_) => Err(de::Error::invalid_value(de::Unexpected::Str(s), &self)),
            }
        }
    }

    de.deserialize_str(StrVisitor(&std::marker::PhantomData))
}

pub struct WrapSSTcp(pub Compat<ProxyClientStream<Compat<TcpStream>>>);

impl AsyncRead for WrapSSTcp {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for WrapSSTcp {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

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
