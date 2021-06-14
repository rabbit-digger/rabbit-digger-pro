use std::{net::SocketAddr, str::FromStr};

use futures::future::pending;
use rd_interface::{
    async_trait,
    registry::ServerFactory,
    schemars::{self, JsonSchema},
    Arc, Config, Error, IServer, Net, Result,
};
use serde_derive::{Deserialize, Serialize};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};
use tokio::select;
use tokio_smoltcp::{device::FutureDevice, BufferSize, NetConfig, TcpListener, UdpSocket};

use dashmap::DashMap;

use crate::{device, gateway::GatewayInterface};

#[derive(Serialize, Deserialize, JsonSchema, Config)]
pub struct RawServerConfig {
    device: String,
    mtu: usize,
    ethernet_addr: String,
    ip_addr: String,
    gateway: String,
}

pub struct RawServer {
    net: Net,
    smoltcp_net: tokio_smoltcp::Net,
    map: Arc<DashMap<SocketAddr, SocketAddr>>,
}

impl RawServer {
    fn new(net: Net, config: RawServerConfig) -> Result<RawServer> {
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
        let device = GatewayInterface::new(device::get_by_device(device)?);
        let map = device.get_map();
        let mut device = FutureDevice::new(device, config.mtu);
        device.caps.max_burst_size = Some(100);

        let (smoltcp_net, fut) = tokio_smoltcp::Net::new(device, net_config);
        tokio::spawn(fut);

        Ok(RawServer {
            net,
            smoltcp_net,
            map,
        })
    }
    async fn serve_tcp(&self, _listener: TcpListener) -> Result<()> {
        pending::<()>().await;
        Ok(())
    }
    async fn serve_udp(&self, udp: UdpSocket) -> Result<()> {
        let mut buf = [0u8; 2048];
        loop {
            let (size, addr) = udp.recv_from(&mut buf).await?;
            // let buf = &buf[..size];
            tracing::trace!("recv from {} size {}", addr, size);
        }
    }
}

#[async_trait]
impl IServer for RawServer {
    async fn start(&self) -> Result<()> {
        let tcp_listener = self
            .smoltcp_net
            .tcp_bind("0.0.0.0:10000".parse().unwrap())
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
