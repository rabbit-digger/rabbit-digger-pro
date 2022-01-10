use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{ready, SinkExt};
use rd_interface::{
    async_trait, error::map_other, AsyncRead, AsyncWrite, ITcpStream, ReadBuf, Result,
};
use tokio::sync::mpsc;
use tokio_util::sync::PollSender;

/// A `ITcpStream` implementation that uses a mpsc channel to send and receive data.
pub struct TcpChannel {
    peer_addr: Option<SocketAddr>,
    local_addr: Option<SocketAddr>,
    sender: PollSender<Vec<u8>>,
    receiver: mpsc::Receiver<Vec<u8>>,
}

impl TcpChannel {
    pub fn new(sender: mpsc::Sender<Vec<u8>>, receiver: mpsc::Receiver<Vec<u8>>) -> TcpChannel {
        TcpChannel {
            peer_addr: None,
            local_addr: None,
            sender: PollSender::new(sender),
            receiver,
        }
    }
    pub fn set_peer_addr(&mut self, addr: SocketAddr) {
        self.peer_addr = Some(addr);
    }
    pub fn set_local_addr(&mut self, addr: SocketAddr) {
        self.local_addr = Some(addr);
    }
}

impl AsyncRead for TcpChannel {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let result = ready!(self.receiver.poll_recv(cx));
        match result {
            Some(data) => {
                let to_read = buf.remaining().min(data.len());
                buf.initialize_unfilled_to(to_read)
                    .copy_from_slice(&data[..to_read]);
                buf.advance(to_read);
                Poll::Ready(Ok(()))
            }
            None => Poll::Ready(Ok(())),
        }
    }
}

impl AsyncWrite for TcpChannel {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        ready!(self.sender.poll_ready_unpin(cx)).map_err(map_other)?;
        self.sender
            .start_send_unpin(buf.to_vec())
            .map_err(map_other)?;
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        ready!(self.sender.poll_flush_unpin(cx)).map_err(map_other)?;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        ready!(Pin::new(&mut self).poll_flush(cx))?;
        self.receiver.close();
        Poll::Ready(Ok(()))
    }
}

#[async_trait]
impl ITcpStream for TcpChannel {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        self.peer_addr.ok_or(rd_interface::NOT_IMPLEMENTED)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.local_addr.ok_or(rd_interface::NOT_IMPLEMENTED)
    }
}
