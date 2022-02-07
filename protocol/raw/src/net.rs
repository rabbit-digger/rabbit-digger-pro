use std::{
    io,
    net::{IpAddr, SocketAddrV4},
    str::FromStr,
};

use crate::{
    config::RawNetConfig,
    device,
    forward::forward_net,
    gateway::GatewayDevice,
    wrap::{TcpListenerWrap, TcpStreamWrap, UdpSocketWrap},
};
use rd_interface::{
    async_trait, registry::NetBuilder, Address, Arc, Context, Error, INet, IntoDyn, Result,
};
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_smoltcp::{
    smoltcp::wire::{IpAddress, IpCidr},
    BufferSize, Net as SmoltcpNet, NetConfig,
};

pub struct RawNet {
    smoltcp_net: Arc<SmoltcpNet>,
    forward_handle: Option<JoinHandle<io::Result<()>>>,
}

impl RawNet {
    fn new(config: RawNetConfig) -> Result<RawNet> {
        let ip_cidr = IpCidr::from_str(&config.ip_addr)
            .map_err(|_| Error::Other("Failed to parse ip_addr".into()))?;
        let ip_addr = match IpAddr::from(ip_cidr.address()) {
            IpAddr::V4(v4) => SocketAddrV4::new(v4, 1),
            IpAddr::V6(_) => return Err(Error::Other("RawNet only support IPv4".into())),
        };
        let gateway = config
            .gateway
            .as_ref()
            .map(|gateway| {
                IpAddress::from_str(&gateway)
                    .map_err(|_| Error::Other("Failed to parse gateway".into()))
            })
            .transpose()?;
        let (ethernet_addr, device) = device::get_device(&config)?;

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
        let mut forward_handle = None;

        let smoltcp_net = if config.forward {
            let device = GatewayDevice::new(device, ethernet_addr, 100, ip_cidr, ip_addr);
            let map = device.get_map();
            let smoltcp_net = Arc::new(SmoltcpNet::new(device, net_config));

            forward_handle = Some(tokio::spawn(forward_net(
                net,
                smoltcp_net.clone(),
                map,
                ip_cidr,
            )));
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

impl NetBuilder for RawNet {
    const NAME: &'static str = "raw";

    type Config = RawNetConfig;
    type Net = RawNet;

    fn build(config: Self::Config) -> Result<Self::Net> {
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
