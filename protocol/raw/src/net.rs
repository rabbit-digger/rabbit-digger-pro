use std::str::FromStr;

use crate::{
    device,
    wrap::{TcpListenerWrap, TcpStreamWrap, UdpSocketWrap},
};
use rd_interface::{
    async_trait, prelude::*, registry::NetFactory, Address, Context, Error, INet, IntoDyn, Result,
};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};
use tokio::sync::Mutex;
use tokio_smoltcp::{device::FutureDevice, BufferSize, Net, NetConfig};

#[rd_config]
pub struct RawNetConfig {
    device: String,
    mtu: usize,
    ethernet_addr: String,
    ip_addr: String,
    gateway: String,
}

pub struct RawNet {
    net: Net,
}

impl RawNet {
    fn new(config: RawNetConfig) -> Result<RawNet> {
        let ethernet_addr = EthernetAddress::from_str(&config.ethernet_addr)
            .map_err(|_| Error::Other("Failed to parse ethernet_addr".into()))?;
        let ip_addr = IpCidr::from_str(&config.ip_addr)
            .map_err(|_| Error::Other("Failed to parse ip_addr".into()))?;
        let gateway = IpAddress::from_str(&config.gateway)
            .map_err(|_| Error::Other("Failed to parse gateway".into()))?;
        let device = device::get_device(&config.device)?;

        let net_config = NetConfig {
            ethernet_addr,
            ip_addr,
            gateway: vec![gateway],
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
        let mut device = FutureDevice::new(device::get_by_device(device)?, config.mtu);
        device.caps.max_burst_size = Some(100);

        let (net, fut) = Net::new(device, net_config);
        tokio::spawn(fut);

        Ok(RawNet { net })
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
        addr: Address,
    ) -> Result<rd_interface::TcpStream> {
        let tcp = TcpStreamWrap(self.net.tcp_connect(addr.to_socket_addr()?).await?);

        Ok(tcp.into_dyn())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut Context,
        addr: Address,
    ) -> Result<rd_interface::TcpListener> {
        let addr = addr.to_socket_addr()?;
        let listener = TcpListenerWrap(Mutex::new(self.net.tcp_bind(addr).await?), addr);
        Ok(listener.into_dyn())
    }

    async fn udp_bind(&self, _ctx: &mut Context, addr: Address) -> Result<rd_interface::UdpSocket> {
        let udp = UdpSocketWrap(self.net.udp_bind(addr.to_socket_addr()?).await?);
        Ok(udp.into_dyn())
    }
}
