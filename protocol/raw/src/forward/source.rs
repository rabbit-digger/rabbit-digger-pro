use std::{
    io,
    net::{SocketAddr, SocketAddrV4},
    pin::Pin,
    task,
};

use futures::{ready, Sink, Stream};
use rd_interface::{Bytes, Result};
use rd_std::util::forward_udp::{self, RawUdpSource};
use tokio_smoltcp::{
    smoltcp::{
        self,
        phy::ChecksumCapabilities,
        wire::{IpCidr, IpProtocol, Ipv4Address, Ipv4Packet, Ipv4Repr, UdpPacket, UdpRepr},
    },
    RawSocket,
};

pub struct Source {
    raw: RawSocket,
    recv_buf: Box<[u8]>,
    send_buf: Option<Vec<u8>>,
    ip_cidr: IpCidr,
}

impl Source {
    pub fn new(raw: RawSocket, ip_cidr: IpCidr) -> Source {
        Source {
            raw,
            recv_buf: Box::new([0u8; 65536]),
            send_buf: None,
            ip_cidr,
        }
    }
}

impl Stream for Source {
    type Item = io::Result<forward_udp::UdpPacket>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let Source {
            raw,
            recv_buf,
            ip_cidr,
            ..
        } = &mut *self;

        let (from, to, data) = loop {
            let size = ready!(raw.poll_recv(cx, recv_buf))?;

            match parse_udp(&recv_buf[..size]) {
                Ok(v) => {
                    let broadcast = match ip_cidr {
                        IpCidr::Ipv4(v4) => {
                            v4.broadcast().map(Into::into).map(std::net::IpAddr::V4)
                        }
                        _ => None,
                    };

                    let to = v.1;
                    if broadcast == Some(to.ip()) || to.ip().is_multicast() {
                        continue;
                    }

                    break v;
                }
                _ => {}
            };
        };

        let data = Bytes::copy_from_slice(data);

        Some(Ok(forward_udp::UdpPacket { from, to, data })).into()
    }
}

impl Sink<forward_udp::UdpPacket> for Source {
    type Error = io::Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        if self.send_buf.is_some() {
            return self.poll_flush(cx);
        }

        Ok(()).into()
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        forward_udp::UdpPacket { from, to, data }: forward_udp::UdpPacket,
    ) -> Result<(), Self::Error> {
        if let Some(ip_packet) = pack_udp(from, to, &data) {
            self.send_buf = Some(ip_packet);
        } else {
            tracing::debug!("Unsupported src/dst");
        }
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        let Source { raw, send_buf, .. } = &mut *self;

        match send_buf {
            Some(buf) => {
                ready!(raw.poll_send(cx, buf))?;
                *send_buf = None;
            }
            None => {}
        }

        Ok(()).into()
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}

impl RawUdpSource for Source {}

/// buf is a ip packet
fn parse_udp(buf: &[u8]) -> smoltcp::Result<(SocketAddr, SocketAddr, &[u8])> {
    let ipv4 = Ipv4Packet::new_checked(buf)?;
    let udp = UdpPacket::new_checked(ipv4.payload())?;

    let src = SocketAddrV4::new(ipv4.src_addr().into(), udp.src_port());
    let dst = SocketAddrV4::new(ipv4.dst_addr().into(), udp.dst_port());

    Ok((src.into(), dst.into(), udp.payload()))
}

fn pack_udp(src: SocketAddr, dst: SocketAddr, payload: &[u8]) -> Option<Vec<u8>> {
    match (src, dst) {
        (SocketAddr::V4(src_v4), SocketAddr::V4(dst_v4)) => {
            let checksum = &ChecksumCapabilities::default();
            let udp_repr = UdpRepr {
                src_port: src.port(),
                dst_port: dst.port(),
            };
            let ipv4_repr = Ipv4Repr {
                src_addr: Ipv4Address::from(*src_v4.ip()),
                dst_addr: Ipv4Address::from(*dst_v4.ip()),
                protocol: IpProtocol::Udp,
                payload_len: udp_repr.header_len() + payload.len(),
                hop_limit: 64,
            };

            let mut buffer =
                vec![0u8; ipv4_repr.buffer_len() + udp_repr.header_len() + payload.len()];

            let mut udp_packet = UdpPacket::new_unchecked(&mut buffer[ipv4_repr.buffer_len()..]);
            udp_repr.emit(
                &mut udp_packet,
                &src.ip().into(),
                &dst.ip().into(),
                payload.len(),
                |buf| buf.copy_from_slice(payload),
                checksum,
            );

            let mut ipv4_packet = Ipv4Packet::new_unchecked(&mut buffer);
            ipv4_repr.emit(&mut ipv4_packet, checksum);

            Some(buffer)
        }
        _ => None,
    }
}
