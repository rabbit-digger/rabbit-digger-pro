use std::{
    io,
    pin::Pin,
    task::{self, Poll},
};

use bytes::BytesMut;
use futures::{ready, Sink, SinkExt, Stream, StreamExt};
use rd_interface::UdpSocket;
use rd_std::util::forward_udp::{RawUdpSource, UdpPacket};
use shadowsocks::crypto::v1::CipherKind;
use socks5_protocol::Address as S5Addr;

use crate::udp::{decrypt_payload, encrypt_payload};

pub struct UdpSource {
    udp: UdpSocket,

    method: CipherKind,
    key: Box<[u8]>,
}

impl UdpSource {
    pub fn new(method: CipherKind, key: Box<[u8]>, udp: UdpSocket) -> UdpSource {
        UdpSource { udp, method, key }
    }
}

impl Stream for UdpSource {
    type Item = io::Result<UdpPacket>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        // TODO: support send_to domain
        let packet = loop {
            let (mut recv_buf, from) = match ready!(self.udp.poll_next_unpin(cx)) {
                Some(r) => r?,
                None => return Poll::Ready(None),
            };
            let (n, addr) = decrypt_payload(self.method, &self.key, &mut recv_buf[..])?;
            // drop the packet if it's sent to domain silently
            let to = match addr {
                S5Addr::Domain(_, _) => continue,
                S5Addr::SocketAddr(s) => s,
            };

            break UdpPacket {
                data: recv_buf.split_to(n).freeze(),
                from,
                to,
            };
        };

        Poll::Ready(Some(Ok(packet)))
    }
}

impl Sink<UdpPacket> for UdpSource {
    type Error = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.udp.poll_ready_unpin(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        UdpPacket { from, to, data }: UdpPacket,
    ) -> Result<(), Self::Error> {
        let mut send_buf = BytesMut::new();

        encrypt_payload(self.method, &self.key, &from.into(), &data, &mut send_buf)?;

        self.udp.start_send_unpin((send_buf.freeze(), to.into()))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.udp.poll_flush_unpin(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.udp.poll_close_unpin(cx)
    }
}

impl RawUdpSource for UdpSource {}
