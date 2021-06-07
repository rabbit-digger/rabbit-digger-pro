use std::{net::SocketAddr, str::FromStr};

use rd_interface::{
    async_trait, impl_async_read_write,
    registry::NetFactory,
    schemars::{self, JsonSchema},
    Address, Config, Context, Error, INet, ITcpStream, IntoDyn, Registry, Result, NOT_IMPLEMENTED,
};
use serde_derive::{Deserialize, Serialize};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};
use tokio_smoltcp::{device::FutureDevice, Net, NetConfig, TcpSocket};

mod device;

pub struct TcpStream(TcpSocket);
impl_async_read_write!(TcpStream, 0);

#[async_trait]
impl ITcpStream for TcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.peer_addr()?)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.local_addr()?)
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Config)]
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
            gateway,
            buffer_size: Default::default(),
        };
        let device = FutureDevice::new(device::get_by_device(device)?, config.mtu);

        let (net, fut) = Net::new(device, net_config);
        tokio::spawn(fut);

        Ok(RawNet { net })
    }
}

#[async_trait]
impl INet for RawNet {
    async fn tcp_connect(
        &self,
        _ctx: &mut Context,
        addr: Address,
    ) -> Result<rd_interface::TcpStream> {
        let tcp = TcpStream(self.net.tcp_connect(addr.to_socket_addr()?).await?);

        Ok(tcp.into_dyn())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut Context,
        _addr: Address,
    ) -> Result<rd_interface::TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(
        &self,
        _ctx: &mut Context,
        _addr: Address,
    ) -> Result<rd_interface::UdpSocket> {
        Err(NOT_IMPLEMENTED)
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

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<RawNet>();

    Ok(())
}
