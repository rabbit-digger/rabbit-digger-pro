use std::{
    io,
    net::{IpAddr, SocketAddrV4},
    str::FromStr,
};

use crate::{
    config::Layer,
    device,
    forward::forward_net,
    gateway::GatewayDevice,
    wrap::{TcpListenerWrap, TcpStreamWrap, UdpSocketWrap},
};
use rd_interface::{
    async_trait, config::NetRef, error::map_other, prelude::*, registry::NetFactory, Address, Arc,
    Context, Error, INet, IntoDyn, Result,
};
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_smoltcp::{
    smoltcp::wire::{EthernetAddress, IpAddress, IpCidr},
    BufferSize, Net as SmoltcpNet, NetConfig,
};

#[rd_config]
pub struct RawNetConfig {
    #[serde(default)]
    net: NetRef,
    device: String,
    mtu: usize,
    ethernet_addr: Option<String>,
    ip_addr: String,
    gateway: Option<String>,

    #[serde(default)]
    forward: bool,
}

pub struct RawNet {
    smoltcp_net: Arc<SmoltcpNet>,
    forward_handle: Option<JoinHandle<io::Result<()>>>,
}

impl RawNet {
    fn new(config: RawNetConfig) -> Result<RawNet> {
        let ethernet_addr = match config.ethernet_addr {
            Some(addr) => EthernetAddress::from_str(&addr)
                .map_err(|_| Error::Other("Failed to parse ethernet_addr".into()))?,
            None => {
                crate::device::get_interface_info(&config.device)
                    .map_err(map_other)?
                    .ethernet_address
            }
        };
        let ip_cidr = IpCidr::from_str(&config.ip_addr)
            .map_err(|_| Error::Other("Failed to parse ip_addr".into()))?;
        let ip_addr = match IpAddr::from(ip_cidr.address()) {
            IpAddr::V4(v4) => SocketAddrV4::new(v4, 1),
            IpAddr::V6(_) => return Err(Error::Other("RawNet only support IPv4".into())),
        };
        let gateway = config
            .gateway
            .map(|gateway| {
                IpAddress::from_str(&gateway)
                    .map_err(|_| Error::Other("Failed to parse gateway".into()))
            })
            .transpose()?;
        let device = device::get_device_name(&config.device)?;

        let net_config = NetConfig {
            ethernet_addr,
            ip_addr: ip_cidr,
            gateway: gateway.into_iter().collect(),
            buffer_size: BufferSize {
                tcp_rx_size: 65536,
                tcp_tx_size: 65536,
                udp_rx_size: 65536,
                udp_tx_size: 65536,
                udp_rx_meta_size: 256,
                udp_tx_meta_size: 256,
                ..Default::default()
            },
        };

        let net = (*config.net).clone();
        let device = device::get_by_device(device)?;
        let mut forward_handle = None;

        let smoltcp_net = if config.forward {
            let device =
                GatewayDevice::new(device, ethernet_addr, 100, ip_cidr, ip_addr, Layer::L2);
            let map = device.get_map();
            let smoltcp_net = Arc::new(SmoltcpNet::new(device, net_config));

            forward_handle = Some(tokio::spawn(forward_net(net, smoltcp_net.clone(), map)));
            smoltcp_net
        } else {
            Arc::new(SmoltcpNet::new(device, net_config))
        };

        Ok(RawNet {
            smoltcp_net,
            forward_handle,
        })
    }
}

impl Drop for RawNet {
    fn drop(&mut self) {
        if let Some(handle) = self.forward_handle.take() {
            handle.abort();
        }
    }
}

impl NetFactory for RawNet {
    const NAME: &'static str = "raw";

    type Config = RawNetConfig;
    type Net = RawNet;

    fn new(config: Self::Config) -> Result<Self::Net> {
        RawNet::new(config)
    }
}

#[async_trait]
impl INet for RawNet {
    async fn tcp_connect(
        &self,
        _ctx: &mut Context,
        addr: &Address,
    ) -> Result<rd_interface::TcpStream> {
        let tcp = TcpStreamWrap(self.smoltcp_net.tcp_connect(addr.to_socket_addr()?).await?);

        Ok(tcp.into_dyn())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut Context,
        addr: &Address,
    ) -> Result<rd_interface::TcpListener> {
        let addr = addr.to_socket_addr()?;
        let listener = TcpListenerWrap(Mutex::new(self.smoltcp_net.tcp_bind(addr).await?), addr);

        Ok(listener.into_dyn())
    }

    async fn udp_bind(
        &self,
        _ctx: &mut Context,
        addr: &Address,
    ) -> Result<rd_interface::UdpSocket> {
        let udp = UdpSocketWrap::new(self.smoltcp_net.udp_bind(addr.to_socket_addr()?).await?);

        Ok(udp.into_dyn())
    }
}
