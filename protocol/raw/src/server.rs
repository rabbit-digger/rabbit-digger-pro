use std::{
    io,
    net::{SocketAddr, SocketAddrV4},
    pin::Pin,
    str::FromStr,
    task,
};

use crate::{
    device,
    gateway::{GatewayDevice, MapTable},
};
use futures::{ready, Sink, Stream, StreamExt};
use rd_interface::{
    async_trait, error::map_other, prelude::*, registry::ServerFactory, Bytes, Context, Error,
    IServer, IntoAddress, Net, Result,
};
use rd_std::util::{
    connect_tcp,
    forward_udp::{self, RawUdpSource},
};
use tokio::{select, spawn};
use tokio_smoltcp::{
    device::{AsyncDevice, Packet},
    smoltcp::{
        self,
        phy::{Checksum, ChecksumCapabilities, Medium},
        wire::{
            EthernetAddress, EthernetFrame, EthernetProtocol, IpCidr, IpProtocol, IpVersion,
            Ipv4Address, Ipv4Packet, Ipv4Repr, UdpPacket, UdpRepr,
        },
    },
    BufferSize, NetConfig, RawSocket, TcpListener,
};

#[rd_config]
pub struct RawServerConfig {
    pub device: String,
    pub mtu: usize,
    /// ipcidr
    pub ip_addr: String,
    pub ethernet_addr: Option<String>,
    #[serde(default = "default_lru")]
    pub lru_size: usize,
    #[serde(default)]
    pub layer: Layer,
}

pub fn default_lru() -> usize {
    128
}

pub struct RawServer {
    net: Net,
    smoltcp_net: tokio_smoltcp::Net,
    map: MapTable,
}

fn filter_packet(
    packet: &[u8],
    ethernet_addr: EthernetAddress,
    ip_addr: IpCidr,
    layer: Layer,
) -> bool {
    let cb = |payload_mut: &[u8]| {
        Ipv4Packet::new_checked(payload_mut)
            .map(|p| {
                ip_addr.contains_addr(&p.src_addr().into())
                    || ip_addr.contains_addr(&p.dst_addr().into())
            })
            .unwrap_or(false)
    };

    match layer {
        Layer::L2 => {
            let f = match EthernetFrame::new_checked(&packet) {
                Ok(p) => p,
                Err(_) => return false,
            };
            let ether_accept = f.dst_addr() == ethernet_addr || f.dst_addr().is_broadcast();

            ether_accept
                && match f.ethertype() {
                    EthernetProtocol::Ipv4 => cb(f.payload()),
                    // ignore other ether type
                    _ => true,
                }
        }
        Layer::L3 => cb(&packet[..]),
    }
}

impl RawServer {
    pub fn new_device(
        net: Net,
        dev_name: &str,
        dev: impl AsyncDevice + 'static,
        ethernet_addr: Option<EthernetAddress>,
        ip_addr: IpCidr,
        lru_size: usize,
        mtu: usize,
        layer: Layer,
    ) -> Result<RawServer> {
        let ethernet_addr = match (ethernet_addr, layer) {
            (Some(ethernet_addr), _) => ethernet_addr,
            (None, Layer::L2) => {
                crate::device::get_interface_info(dev_name)
                    .map_err(map_other)?
                    .ethernet_address
            }
            (None, Layer::L3) => EthernetAddress::BROADCAST,
        };
        let gateway = ip_addr.address();
        let net_config = NetConfig {
            ethernet_addr,
            ip_addr,
            gateway: vec![gateway],
            buffer_size: BufferSize {
                tcp_rx_size: 65536,
                tcp_tx_size: 65536,
                raw_rx_size: 65536,
                raw_tx_size: 65536,
                raw_rx_meta_size: 256,
                raw_tx_meta_size: 256,
                ..Default::default()
            },
        };

        let addr = match ip_addr {
            IpCidr::Ipv4(v4) => v4.address(),
            _ => return Err(Error::Other("Ipv6 is not supported".into())),
        };

        let device = GatewayDevice::new(
            dev.filter(move |p: &Packet| {
                std::future::ready(filter_packet(p, ethernet_addr, ip_addr, layer))
            }),
            lru_size,
            SocketAddrV4::new(addr.into(), 20000),
            layer,
        );
        let map = device.get_map();
        let mut device = FutureDevice::new(device, mtu);
        match layer {
            Layer::L2 => device.caps.medium = Medium::Ethernet,
            Layer::L3 => device.caps.medium = Medium::Ip,
        }
        device.caps.max_burst_size = Some(100);
        // ignored checksum since we modify the packets
        device.caps.checksum = ChecksumCapabilities::ignored();
        device.caps.checksum.ipv4 = Checksum::Tx;

        let smoltcp_net = tokio_smoltcp::Net::new(device, net_config);

        Ok(RawServer {
            net,
            smoltcp_net,
            map,
        })
    }
    pub fn new(net: Net, config: RawServerConfig) -> Result<RawServer> {
        let ethernet_addr = match config.ethernet_addr {
            Some(ethernet_addr) => Some(
                EthernetAddress::from_str(&ethernet_addr)
                    .map_err(|_| Error::Other("Failed to parse ethernet_addr".into()))?,
            ),
            None => None,
        };
        let ip_addr = IpCidr::from_str(&config.ip_addr)
            .map_err(|_| Error::Other("Failed to parse ip_addr".into()))?;

        let device = device::get_device_name(&config.device)?;
        let device = device::get_by_device(device)?;

        Self::new_device(
            net,
            &config.device,
            device,
            ethernet_addr,
            ip_addr,
            config.lru_size,
            config.mtu,
            config.layer,
        )
    }
    async fn serve_tcp(&self, mut listener: TcpListener) -> Result<()> {
        loop {
            let (tcp, addr) = listener.accept().await?;
            let orig_addr = self.map.get(&match addr {
                SocketAddr::V4(v4) => v4,
                _ => continue,
            });
            if let Some(orig_addr) = orig_addr {
                let net = self.net.clone();
                spawn(async move {
                    let ctx = &mut Context::from_socketaddr(addr);
                    let target = net
                        .tcp_connect(ctx, &SocketAddr::from(orig_addr).into_address()?)
                        .await?;
                    connect_tcp(ctx, tcp, target).await?;
                    Ok(()) as Result<()>
                });
            }
        }
    }
    async fn serve_udp(&self, raw: RawSocket) -> Result<()> {
        let source = Source::new(raw);

        forward_udp::forward_udp(source, self.net.clone()).await?;

        Ok(())
    }
}

struct Source {
    raw: RawSocket,
    recv_buf: Box<[u8]>,
    send_buf: Option<Vec<u8>>,
}

impl Source {
    pub fn new(raw: RawSocket) -> Source {
        Source {
            raw,
            recv_buf: Box::new([0u8; 65536]),
            send_buf: None,
        }
    }
}

impl Stream for Source {
    type Item = io::Result<forward_udp::UdpPacket>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let Source { raw, recv_buf, .. } = &mut *self;

        let (from, to, data) = loop {
            let size = ready!(raw.poll_recv(cx, recv_buf))?;

            match parse_udp(&recv_buf[..size]) {
                Ok(v) => break v,
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

#[async_trait]
impl IServer for RawServer {
    async fn start(&self) -> Result<()> {
        let tcp_listener = self
            .smoltcp_net
            .tcp_bind("0.0.0.0:20000".parse().map_err(map_other)?)
            .await?;
        let raw_socket = self
            .smoltcp_net
            .raw_socket(IpVersion::Ipv4, IpProtocol::Udp)
            .await?;

        let tcp_task = self.serve_tcp(tcp_listener);
        let udp_task = self.serve_udp(raw_socket);

        select! {
            r = tcp_task => r?,
            r = udp_task => r?,
        };

        Ok(())
    }
}

impl ServerFactory for RawServer {
    const NAME: &'static str = "raw";

    type Config = RawServerConfig;
    type Server = RawServer;

    fn new(_listen: Net, net: Net, config: Self::Config) -> Result<Self::Server> {
        RawServer::new(net, config)
    }
}
