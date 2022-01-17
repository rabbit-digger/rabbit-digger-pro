use std::{io, net::SocketAddr, pin::Pin, task};

use super::UdpPacket;
use futures::{ready, Sink, SinkExt, Stream};
use rd_interface::{Address, Bytes, IUdpChannel};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::sync::PollSender;

pub(super) struct BackChannel {
    to: SocketAddr,
    sender: PollSender<UdpPacket>,
    receiver: Receiver<(Bytes, SocketAddr)>,
}

impl BackChannel {
    pub(super) fn new(
        to: SocketAddr,
        sender: Sender<UdpPacket>,
        receiver: Receiver<(Bytes, SocketAddr)>,
    ) -> BackChannel {
        BackChannel {
            to,
            sender: PollSender::new(sender),
            receiver,
        }
    }
}

impl IUdpChannel for BackChannel {}

impl Stream for BackChannel {
    type Item = io::Result<(Bytes, Address)>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let item = ready!(self.receiver.poll_recv(cx));
        task::Poll::Ready(item.map(|(buf, addr)| Ok((buf, addr.into()))))
    }
}

impl Sink<(Bytes, SocketAddr)> for BackChannel {
    type Error = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        let result = ready!(self.sender.poll_ready_unpin(cx));
        result
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .into()
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        (data, from): (Bytes, SocketAddr),
    ) -> Result<(), Self::Error> {
        let to = self.to;
        self.sender
            .start_send_unpin(UdpPacket { from, to, data })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        let result = ready!(self.sender.poll_flush_unpin(cx));
        result
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .into()
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        let result = ready!(self.sender.poll_close_unpin(cx));
        result
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .into()
    }
}
