use rd_interface::{async_trait, AsyncRead, AsyncWrite, ReadBuf};
use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use super::event::{Event, EventType};

pub struct TcpStream {
    inner: rd_interface::TcpStream,
    sender: UnboundedSender<Event>,
    uuid: Uuid,
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        self.send(EventType::CloseConnection);
    }
}

impl TcpStream {
    pub fn send(&self, event_type: EventType) {
        if self.sender.send(Event::new(self.uuid, event_type)).is_err() {
            log::warn!("Failed to send event");
        }
    }
    pub fn new(inner: rd_interface::TcpStream, sender: UnboundedSender<Event>) -> TcpStream {
        let uuid = Uuid::new_v4();
        TcpStream {
            inner,
            sender,
            uuid,
        }
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let s = buf.filled().len() - before;
                self.send(EventType::Inbound(s));
                Ok(()).into()
            }
            r => r,
        }
    }
}
impl AsyncWrite for TcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match Pin::new(&mut self.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(s)) => {
                self.send(EventType::Outbound(s));
                Ok(s).into()
            }
            r => r,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[async_trait]
impl rd_interface::ITcpStream for TcpStream {
    async fn peer_addr(&self) -> rd_interface::Result<SocketAddr> {
        self.inner.peer_addr().await
    }

    async fn local_addr(&self) -> rd_interface::Result<SocketAddr> {
        self.inner.local_addr().await
    }
}
