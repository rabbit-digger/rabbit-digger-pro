use std::pin::Pin;

use super::TlsConnectorConfig;
use openssl::ssl::{SslConnector, SslMethod};
use openssl_crate as openssl;
use rd_interface::{error::map_other, AsyncRead, AsyncWrite, Result};

pub use tokio_openssl::SslStream as TlsStream;

pub struct TlsConnector {
    connector: SslConnector,
    sni: String,
}

impl TlsConnector {
    pub fn new(config: TlsConnectorConfig) -> Result<TlsConnector> {
        let connector = SslConnector::builder(SslMethod::tls())
            .map_err(map_other)?
            .build();

        Ok(TlsConnector {
            connector,
            sni: config.sni,
        })
    }

    pub async fn connect<IO>(&self, stream: IO) -> Result<TlsStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let ssl = self
            .connector
            .configure()
            .map_err(map_other)?
            .into_ssl(&self.sni)
            .map_err(map_other)?;

        let mut stream = TlsStream::new(ssl, stream).map_err(map_other)?;

        Pin::new(&mut stream).connect().await.map_err(map_other)?;

        Ok(stream)
    }
}
