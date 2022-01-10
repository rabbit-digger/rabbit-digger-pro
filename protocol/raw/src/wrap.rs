use std::{io, net::SocketAddr, pin::Pin, task};

use futures::{ready, Sink, Stream};
use rd_interface::{
    async_trait, constant::UDP_BUFFER_SIZE, impl_async_read_write, Bytes, BytesMut, ITcpListener,
    ITcpStream, IUdpSocket, IntoDyn, Result,
};
use tokio::sync::Mutex;
use tokio_smoltcp::{TcpListener, TcpSocket, UdpSocket};

pub struct TcpStreamWrap(pub(crate) TcpSocket);
impl_async_read_write!(TcpStreamWrap, 0);

#[async_trait]
impl ITcpStream for TcpStreamWrap {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.peer_addr()?)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.local_addr()?)
    }
}

pub struct TcpListenerWrap(pub(crate) Mutex<TcpListener>, pub(crate) SocketAddr);

#[async_trait]
impl ITcpListener for TcpListenerWrap {
    async fn accept(&self) -> Result<(rd_interface::TcpStream, SocketAddr)> {
        let (tcp, addr) = self.0.lock().await.accept().await?;
        Ok((TcpStreamWrap(tcp).into_dyn(), addr))
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.1)
    }
}

pub struct UdpSocketWrap {
    inner: UdpSocket,
    recv_buf: Box<[u8]>,
    send_buf: Option<(Bytes, SocketAddr)>,
}

impl UdpSocketWrap {
    pub(crate) fn new(inner: UdpSocket) -> Self {
        Self {
            inner,
            recv_buf: vec![0; UDP_BUFFER_SIZE].into_boxed_slice(),
            send_buf: None,
        }
    }
}

impl Stream for UdpSocketWrap {
    type Item = io::Result<(BytesMut, SocketAddr)>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let UdpSocketWrap {
            inner,
            recv_buf: buf,
            ..
        } = &mut *self;
        let (size, from) = ready!(inner.poll_recv_from(cx, buf))?;
        let buf = BytesMut::from(&buf[..size]);
        Some(Ok((buf, from))).into()
    }
}

impl Sink<(Bytes, SocketAddr)> for UdpSocketWrap {
    type Error = io::Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        if self.send_buf.is_some() {
            ready!(self.poll_flush(cx))?;
        }
        Ok(()).into()
    }

    fn start_send(mut self: Pin<&mut Self>, item: (Bytes, SocketAddr)) -> Result<(), Self::Error> {
        self.send_buf = Some(item);

        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        let UdpSocketWrap {
            inner,
            send_buf: buf,
            ..
        } = &mut *self;
        if let Some((buf, to)) = buf {
            let size = ready!(inner.poll_send_to(cx, buf, *to))?;
            if size != buf.len() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "failed to send all bytes",
                ))
                .into();
            }
        }
        Ok(()).into()
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        ready!(self.poll_flush(cx))?;
        Ok(()).into()
    }
}

#[async_trait]
impl IUdpSocket for UdpSocketWrap {
    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.inner.local_addr()?)
    }
}
