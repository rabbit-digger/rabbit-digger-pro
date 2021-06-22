use std::{
    net::SocketAddrV4,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures::{ready, Sink, SinkExt, Stream, StreamExt};
use lru_time_cache::LruCache;
use parking_lot::Mutex;
use rd_interface::Result;
use smoltcp::wire::{
    EthernetFrame, EthernetProtocol, IpProtocol, Ipv4Packet, TcpPacket, UdpPacket,
};
use tokio_smoltcp::device::{Interface, Packet};

#[derive(Clone)]
pub struct MapTable {
    map: Arc<Mutex<LruCache<SocketAddrV4, SocketAddrV4>>>,
}

impl MapTable {
    fn new(cap: usize) -> MapTable {
        MapTable {
            map: Arc::new(Mutex::new(LruCache::with_capacity(cap))),
        }
    }
    fn insert(&self, key: SocketAddrV4, value: SocketAddrV4) {
        self.map.lock().insert(key, value);
    }
    pub fn get(&self, key: &SocketAddrV4) -> Option<SocketAddrV4> {
        self.map.lock().get(key).copied()
    }
}

pub struct GatewayInterface<I> {
    inner: I,
    map: MapTable,

    override_v4: SocketAddrV4,
}

fn set_dst_addr<T: AsRef<[u8]> + AsMut<[u8]>>(
    ip: &mut Ipv4Packet<T>,
    dst_addr: SocketAddrV4,
) -> smoltcp::Result<Option<(SocketAddrV4, SocketAddrV4)>> {
    let src_addr = ip.src_addr();
    let orig_addr = ip.dst_addr();

    let (src_port, orig_port) = match ip.protocol() {
        IpProtocol::Tcp => {
            ip.set_dst_addr(dst_addr.ip().to_owned().into());

            let mut tcp = TcpPacket::new_unchecked(ip.payload_mut());
            let dst_port = tcp.dst_port();
            tcp.set_dst_port(dst_addr.port());

            (tcp.src_port(), dst_port)
        }
        _ => return Ok(None),
    };

    Ok(Some((
        SocketAddrV4::new(src_addr.into(), src_port),
        SocketAddrV4::new(orig_addr.into(), orig_port),
    )))
}

fn get_dst_addr<T: AsRef<[u8]> + AsMut<[u8]>>(
    ip: &mut Ipv4Packet<T>,
) -> smoltcp::Result<Option<SocketAddrV4>> {
    let dst_addr = ip.dst_addr();
    let port = match ip.protocol() {
        IpProtocol::Tcp => TcpPacket::new_checked(ip.payload_mut())?.dst_port(),
        IpProtocol::Udp => UdpPacket::new_checked(ip.payload_mut())?.dst_port(),
        _ => return Ok(None),
    };
    Ok(Some(SocketAddrV4::new(dst_addr.into(), port)))
}

fn set_src_addr<T: AsRef<[u8]> + AsMut<[u8]>>(
    ip: &mut Ipv4Packet<T>,
    src_addr_v4: SocketAddrV4,
) -> smoltcp::Result<()> {
    let src_addr = src_addr_v4.ip().to_owned().into();
    let dst_addr = ip.dst_addr();
    let port = src_addr_v4.port();
    if let IpProtocol::Tcp = ip.protocol() {
        ip.set_src_addr(src_addr);

        let mut tcp = TcpPacket::new_checked(ip.payload_mut())?;
        tcp.set_src_port(port);

        tcp.fill_checksum(&src_addr.into(), &dst_addr.into());
    };
    ip.fill_checksum();

    Ok(())
}

impl<I> GatewayInterface<I> {
    pub fn new(inner: I, lru_size: usize, override_v4: SocketAddrV4) -> GatewayInterface<I> {
        GatewayInterface {
            inner,
            map: MapTable::new(lru_size),

            override_v4,
        }
    }
    pub fn get_map(&self) -> MapTable {
        self.map.clone()
    }
    fn map_in(&self, mut packet: Packet) -> Packet {
        fn map(
            packet: &mut Packet,
            override_v4: SocketAddrV4,
            map_table: &MapTable,
        ) -> smoltcp::Result<()> {
            let mut frame = EthernetFrame::new_checked(packet)?;

            match frame.ethertype() {
                EthernetProtocol::Ipv4 => {
                    let mut ipv4 = Ipv4Packet::new_checked(frame.payload_mut())?;
                    if let Some((src_addr, ori_addr)) = set_dst_addr(&mut ipv4, override_v4)? {
                        map_table.insert(src_addr, ori_addr);
                    }
                }
                _ => return Ok(()),
            };

            Ok(())
        }
        map(&mut packet, self.override_v4, &self.map).ok();
        packet
    }
    fn map_out(&self, mut packet: Packet) -> Packet {
        fn map(packet: &mut Packet, map_table: &MapTable) -> smoltcp::Result<()> {
            let mut frame = EthernetFrame::new_checked(packet)?;

            match frame.ethertype() {
                EthernetProtocol::Ipv4 => {
                    let mut ipv4 = Ipv4Packet::new_checked(frame.payload_mut())?;
                    if let Some(src) = get_dst_addr(&mut ipv4)?
                        .map(|d| map_table.get(&d))
                        .flatten()
                    {
                        set_src_addr(&mut ipv4, src)?;
                    }
                }
                _ => return Ok(()),
            };

            Ok(())
        }
        map(&mut packet, &self.map).ok();
        packet
    }
}

impl<I> Stream for GatewayInterface<I>
where
    I: Interface,
{
    type Item = I::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let item = ready!(self.inner.poll_next_unpin(cx));
        Poll::Ready(item.map(|p| self.map_in(p)))
    }
}

impl<I> Sink<Packet> for GatewayInterface<I>
where
    I: Interface,
{
    type Error = I::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready_unpin(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Packet) -> Result<(), Self::Error> {
        let item = self.map_out(item);
        self.inner.start_send_unpin(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_flush_unpin(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_close_unpin(cx)
    }
}
