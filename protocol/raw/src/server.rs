use std::{
    net::{SocketAddr, SocketAddrV4},
    str::FromStr,
};

use futures::{future::ready, StreamExt};
use rd_interface::{
    async_trait,
    registry::ServerFactory,
    schemars::{self, JsonSchema},
    util::connect_tcp,
    Config, Context, Error, IServer, IntoAddress, Net, Result,
};
use serde_derive::{Deserialize, Serialize};
use smoltcp::wire::{EthernetFrame, EthernetProtocol, Ipv4Packet};
use smoltcp::{
    phy::ChecksumCapabilities,
    wire::{EthernetAddress, IpCidr},
};
use tokio::{select, spawn};
use tokio_smoltcp::{
    device::{FutureDevice, Packet},
    BufferSize, NetConfig, TcpListener, UdpSocket,
};

use crate::{
    device,
    gateway::{GatewayInterface, MapTable},
};

#[derive(Serialize, Deserialize, JsonSchema, Config)]
pub struct RawServerConfig {
    device: String,
    mtu: usize,
    ip_addr: String,
    ethernet_addr: String,
}

pub struct RawServer {
    net: Net,
    smoltcp_net: tokio_smoltcp::Net,
    map: MapTable,
}

fn filter_packet(packet: &Packet, ethernet_addr: EthernetAddress, ip_addr: IpCidr) -> bool {
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
            gateway,
            buffer_size: BufferSize {
                tcp_rx_size: 65536,
                tcp_tx_size: 65536,
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
            SocketAddrV4::new(addr.into(), 20000),
        );
        let map = device.get_map();
        let mut device = FutureDevice::new(device, config.mtu);
        device.caps.max_burst_size = Some(100);
        // ignored checksum since we modify the packets
        device.caps.checksum = ChecksumCapabilities::ignored();

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
                    let target = net
                        .tcp_connect(
                            &mut Context::new(),
                            SocketAddr::from(orig_addr).into_address()?,
                        )
                        .await?;
                    connect_tcp(tcp, target).await?;
                    Ok(()) as Result<()>
                });
            }
        }
    }
    async fn serve_udp(&self, udp: UdpSocket) -> Result<()> {
        let mut buf = [0u8; 2048];
        let nudp = self
            .net
            .udp_bind(&mut Context::new(), "0.0.0.0:0".into_address()?)
            .await?;
        loop {
            let (size, addr) = udp.recv_from(&mut buf).await?;
            let orig_addr = self.map.get(&match addr {
                SocketAddr::V4(v4) => v4,
                _ => continue,
            });
            if let Some(orig_addr) = orig_addr {
                tracing::trace!(
                    "recv from {} size {}, orig_addr {:?}",
                    addr,
                    size,
                    orig_addr
                );
                nudp.send_to(&buf[..size], SocketAddr::from(orig_addr).into_address()?)
                    .await?;

                let (size, recv_addr) = nudp.recv_from(&mut buf).await?;
                tracing::trace!("recv_from other side {}", recv_addr);
                udp.send_to(&buf[..size], addr).await?;
            }
        }
    }
}

#[async_trait]
impl IServer for RawServer {
    async fn start(&self) -> Result<()> {
        let tcp_listener = self
            .smoltcp_net
            .tcp_bind("0.0.0.0:20000".parse().unwrap())
            .await?;
        let udp_listener = self
            .smoltcp_net
            .udp_bind("0.0.0.0:20000".parse().unwrap())
            .await?;

        let tcp_task = self.serve_tcp(tcp_listener);
        let udp_task = self.serve_udp(udp_listener);

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
