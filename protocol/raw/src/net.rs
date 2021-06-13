use std::{net::SocketAddr, str::FromStr};

use crate::device;
use rd_interface::{
    async_trait, impl_async_read_write,
    registry::NetFactory,
    schemars::{self, JsonSchema},
    Address, Config, Context, Error, INet, ITcpListener, ITcpStream, IUdpSocket, IntoDyn, Result,
};
use serde_derive::{Deserialize, Serialize};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};
use tokio::sync::Mutex;
use tokio_smoltcp::{
    device::FutureDevice, BufferSize, Net, NetConfig, TcpListener, TcpSocket, UdpSocket,
};

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
            buffer_size: BufferSize {
                tcp_rx_size: 65536,
                tcp_tx_size: 65536,
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
        let tcp = TcpStream(self.net.tcp_connect(addr.to_socket_addr()?).await?);

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

pub struct TcpListenerWrap(Mutex<TcpListener>, SocketAddr);

#[async_trait]
impl ITcpListener for TcpListenerWrap {
    async fn accept(&self) -> Result<(rd_interface::TcpStream, SocketAddr)> {
        let (tcp, addr) = self.0.lock().await.accept().await?;
        Ok((TcpStream(tcp).into_dyn(), addr))
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.1)
    }
}

pub struct UdpSocketWrap(UdpSocket);

#[async_trait]
impl IUdpSocket for UdpSocketWrap {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        Ok(self.0.recv_from(buf).await?)
    }

    async fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize> {
        Ok(self.0.send_to(buf, addr.to_socket_addr()?).await?)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.local_addr()?)
    }
}
