use std::{
    net::{SocketAddr, SocketAddrV4},
    str::FromStr,
    time::Duration,
};

use futures::{future::ready, StreamExt};
use lru_time_cache::LruCache;
use rd_interface::{
    async_trait, constant::UDP_BUFFER_SIZE, error::map_other, prelude::*, registry::ServerFactory,
    Address, Context, Error, IServer, IntoAddress, Net, Result,
};
use rd_std::util::connect_tcp;
use smoltcp::{
    phy::{Checksum, ChecksumCapabilities},
    wire::{
        EthernetAddress, EthernetFrame, EthernetProtocol, IpCidr, IpProtocol, IpVersion,
        Ipv4Address, Ipv4Packet, Ipv4Repr, UdpPacket, UdpRepr,
    },
};
use tokio::{
    select, spawn,
    sync::mpsc::{unbounded_channel, UnboundedSender as Sender},
    time::timeout,
};
use tokio_smoltcp::{
    device::{FutureDevice, Packet},
    BufferSize, NetConfig, RawSocket, TcpListener,
};

use crate::{
    device,
    gateway::{GatewayInterface, MapTable},
};

#[rd_config]
pub struct RawServerConfig {
    device: String,
    mtu: usize,
    ip_addr: String,
    ethernet_addr: String,
    #[serde(default = "default_lru")]
    lru_size: usize,
}

fn default_lru() -> usize {
    128
}

pub struct RawServer {
    net: Net,
    smoltcp_net: tokio_smoltcp::Net,
    map: MapTable,
}

fn filter_packet(packet: &[u8], ethernet_addr: EthernetAddress, ip_addr: IpCidr) -> bool {
    if let Ok(f) = EthernetFrame::new_checked(packet) {
        let ether_accept = f.dst_addr() == ethernet_addr || f.dst_addr().is_broadcast();
        ether_accept && {
            match (f.ethertype(), Ipv4Packet::new_checked(f.payload())) {
                (EthernetProtocol::Ipv4, Ok(p)) => {
                    ip_addr.contains_addr(&p.src_addr().into())
                        || ip_addr.contains_addr(&p.dst_addr().into())
                }
                // ignore other ether type
                _ => true,
            }
        }
    } else {
        false
    }
}

impl RawServer {
    fn new(net: Net, config: RawServerConfig) -> Result<RawServer> {
        let ethernet_addr = EthernetAddress::from_str(&config.ethernet_addr)
            .map_err(|_| Error::Other("Failed to parse ethernet_addr".into()))?;
        let ip_addr = IpCidr::from_str(&config.ip_addr)
            .map_err(|_| Error::Other("Failed to parse ip_addr".into()))?;
        let gateway = ip_addr.address();
        let device = device::get_device(&config.device)?;

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

        let device = GatewayInterface::new(
            device::get_by_device(device)?
                .filter(move |p: &Packet| ready(filter_packet(p, ethernet_addr, ip_addr))),
            config.lru_size,
            SocketAddrV4::new(addr.into(), 20000),
        );
        let map = device.get_map();
        let mut device = FutureDevice::new(device, config.mtu);
        device.caps.max_burst_size = Some(100);
        // ignored checksum since we modify the packets
        device.caps.checksum = ChecksumCapabilities::ignored();
        device.caps.checksum.ipv4 = Checksum::Tx;

        let (smoltcp_net, fut) = tokio_smoltcp::Net::new(device, net_config);
        tokio::spawn(fut);

        Ok(RawServer {
            net,
            smoltcp_net,
            map,
        })
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
        let (send_raw, mut send_rx) = unbounded_channel::<(SocketAddr, SocketAddr, Vec<u8>)>();

        let mut buf = [0u8; UDP_BUFFER_SIZE];
        let mut nat = LruCache::<SocketAddr, UdpTunnel>::with_expiry_duration_and_capacity(
            Duration::from_secs(30),
            128,
        );
        let net = self.net.clone();

        let recv = async {
            loop {
                let size = raw.recv(&mut buf).await?;
                let (src, dst, payload) = match parse_udp(&buf[..size]) {
                    Ok(v) => v,
                    _ => break,
                };

                let udp = nat
                    .entry(src)
                    .or_insert_with(|| UdpTunnel::new(net.clone(), src, send_raw.clone()));
                if let Err(e) = udp.send_to(payload, dst).await {
                    tracing::error!("Udp send_to {:?}", e);
                    nat.remove(&src);
                }
            }

            Ok(()) as Result<()>
        };

        let send = async {
            while let Some((src, dst, payload)) = send_rx.recv().await {
                if let Some(ip_packet) = pack_udp(src, dst, &payload) {
                    if let Err(e) = raw.send(&ip_packet).await {
                        tracing::error!(
                            "Raw send error: {:?}, dropping udp size: {}",
                            e,
                            ip_packet.len()
                        );
                    }
                } else {
                    tracing::debug!("Unsupported src/dst");
                }
            }
            Ok(()) as Result<()>
        };

        select! {
            r = send => r?,
            r = recv => r?,
        }

        Ok(())
    }
}

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
                payload,
            };
            let ipv4_repr = Ipv4Repr {
                src_addr: Ipv4Address::from(*src_v4.ip()),
                dst_addr: Ipv4Address::from(*dst_v4.ip()),
                protocol: IpProtocol::Udp,
                payload_len: udp_repr.buffer_len(),
                hop_limit: 64,
            };

            let mut buffer = vec![0u8; ipv4_repr.buffer_len() + udp_repr.buffer_len()];

            let mut udp_packet = UdpPacket::new_unchecked(&mut buffer[ipv4_repr.buffer_len()..]);
            udp_repr.emit(
                &mut udp_packet,
                &src.ip().into(),
                &dst.ip().into(),
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
            .tcp_bind("0.0.0.0:20000".parse().unwrap())
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

struct UdpTunnel {
    tx: Sender<(SocketAddr, Vec<u8>)>,
}

impl UdpTunnel {
    fn new(
        net: Net,
        src: SocketAddr,
        send_raw: Sender<(SocketAddr, SocketAddr, Vec<u8>)>,
    ) -> UdpTunnel {
        let (tx, mut rx) = unbounded_channel::<(SocketAddr, Vec<u8>)>();
        tokio::spawn(async move {
            let udp = timeout(
                Duration::from_secs(5),
                net.udp_bind(
                    &mut Context::from_socketaddr(src),
                    &Address::any_addr_port(&src),
                ),
            )
            .await
            .map_err(map_other)??;

            let send = async {
                while let Some((addr, packet)) = rx.recv().await {
                    udp.send_to(&packet, addr.into()).await?;
                }
                Ok(())
            };
            let recv = async {
                let mut buf = [0u8; UDP_BUFFER_SIZE];
                loop {
                    let (size, addr) = udp.recv_from(&mut buf).await?;

                    if send_raw.send((addr, src, buf[..size].to_vec())).is_err() {
                        break;
                    }
                }
                tracing::trace!("send_raw return error");
                Ok(())
            };

            let r: Result<()> = select! {
                r = send => r,
                r = recv => r,
            };

            r
        });
        UdpTunnel { tx }
    }
    /// return false if the send queue is full
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<()> {
        match self.tx.send((addr, buf.to_vec())) {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::Other("Other side closed".into())),
        }
    }
}
