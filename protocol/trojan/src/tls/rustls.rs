use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use super::TlsConnectorConfig;
use futures::ready;
use rd_interface::{error::map_other, AsyncRead, AsyncWrite, Result};
use std::sync::Arc;
use tokio::io::ReadBuf;
use tokio_rustls::{
    rustls::{ClientConfig, ServerCertVerified, ServerCertVerifier},
    webpki::{DNSName, DNSNameRef},
};

pub type TlsStream<T> = PushingStream<tokio_rustls::TlsStream<T>>;

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
        Ok(PushingStream::new(stream.into()))
    }
}

enum State {
    Write,
    Flush(usize),
}

pub struct PushingStream<S> {
    inner: S,
    state: State,
}

impl<S> AsyncRead for PushingStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for PushingStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let wrote = loop {
            match self.state {
                State::Write => {
                    let wrote = ready!(Pin::new(&mut self.inner).poll_write(cx, buf))?;
                    self.state = State::Flush(wrote);
                }
                State::Flush(wrote) => {
                    ready!(Pin::new(&mut self.inner).poll_flush(cx))?;
                    self.state = State::Write;
                    break wrote;
                }
            }
        };

        Poll::Ready(Ok(wrote))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

impl<S> PushingStream<S> {
    pub fn new(inner: S) -> Self {
        PushingStream {
            inner,
            state: State::Write,
        }
    }
}
