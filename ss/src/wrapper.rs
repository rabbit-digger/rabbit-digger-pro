use pin::Pin;
use rd_interface::{
    async_trait, Address as RDAddress, AsyncRead, AsyncWrite, ITcpStream, TcpStream,
    NOT_IMPLEMENTED,
};
use serde::{
    de::{self, Visitor},
    Deserializer,
};
use shadowsocks::{crypto::v1::CipherKind, relay::socks5::Address as SSAddress, ProxyClientStream};
use std::{io, net::SocketAddr, pin, task};
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
            RDAddress::IPv4(v4) => SSAddress::SocketAddress(SocketAddr::V4(v4)),
            RDAddress::IPv6(v6) => SSAddress::SocketAddress(SocketAddr::V6(v6)),
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
rc4-md5 chacha20-ietf chacha20-ietf-poly1305"
            )
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(WrapCipher(match s {
                "aes-128-gcm" => CipherKind::AES_128_GCM,
                "aes-256-gcm" => CipherKind::AES_256_GCM,
                "aes-128-cfb" => CipherKind::AES_128_CFB128,
                "aes-192-cfb" => CipherKind::AES_192_CFB128,
                "aes-256-cfb" => CipherKind::AES_256_CFB128,
                "aes-128-ctr" => CipherKind::AES_128_CTR,
                "aes-192-ctr" => CipherKind::AES_192_CTR,
                "aes-256-ctr" => CipherKind::AES_256_CTR,
                "rc4-md5" => CipherKind::SS_RC4_MD5,
                "chacha20-ietf" => CipherKind::CHACHA20,
                "chacha20-ietf-poly1305" => CipherKind::CHACHA20_POLY1305,

                _ => return Err(de::Error::invalid_value(de::Unexpected::Str(s), &self)),
            }))
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

unsafe impl Sync for WrapSSTcp {}
