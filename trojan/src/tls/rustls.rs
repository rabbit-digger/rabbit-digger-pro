use super::TlsConnectorConfig;
use rd_interface::{error::map_other, AsyncRead, AsyncWrite, Result};
use std::sync::Arc;
use tokio_rustls::{
    rustls::{ClientConfig, ServerCertVerified, ServerCertVerifier},
    webpki::{DNSName, DNSNameRef},
};

pub use tokio_rustls::TlsStream;

struct AllowAnyCert;
impl ServerCertVerifier for AllowAnyCert {
    fn verify_server_cert(
        &self,
        _roots: &tokio_rustls::rustls::RootCertStore,
        _presented_certs: &[tokio_rustls::rustls::Certificate],
        _dns_name: DNSNameRef,
        _ocsp_response: &[u8],
    ) -> Result<ServerCertVerified, tokio_rustls::rustls::TLSError> {
        Ok(ServerCertVerified::assertion())
    }
}

pub struct TlsConnector {
    sni: DNSName,
    connector: tokio_rustls::TlsConnector,
}

impl TlsConnector {
    pub fn new(config: TlsConnectorConfig) -> Result<TlsConnector> {
        let mut client_config = ClientConfig::default();
        if config.skip_cert_verify {
            client_config
                .dangerous()
                .set_certificate_verifier(Arc::new(AllowAnyCert));
        } else {
            client_config
                .root_store
                .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
        }
        let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));
        let sni = DNSNameRef::try_from_ascii_str(&config.sni)
            .map_err(map_other)?
            .into();
        Ok(TlsConnector { sni, connector })
    }
    pub async fn connect<IO>(&self, stream: IO) -> Result<TlsStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let stream = self.connector.connect(self.sni.as_ref(), stream).await?;
        Ok(stream.into())
    }
}
