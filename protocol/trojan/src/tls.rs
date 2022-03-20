#[cfg(feature = "rustls")]
#[path = "tls/rustls.rs"]
mod backend;

#[cfg(feature = "openssl")]
#[path = "tls/openssl.rs"]
mod backend;

#[cfg(feature = "native-tls")]
#[path = "tls/native-tls.rs"]
mod backend;

pub use backend::*;

#[derive(Clone)]
pub struct TlsConnectorConfig {
    pub skip_cert_verify: bool,
    pub sni: String,
}
