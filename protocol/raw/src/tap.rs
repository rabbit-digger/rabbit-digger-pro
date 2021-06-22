use std::{
    future::ready,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

use crate::server::RawServer;
use futures::{SinkExt, StreamExt};
use rd_interface::{
    async_trait, error::map_other, prelude::*, registry::ServerFactory, Error, IServer, Net, Result,
};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};
use tokio_smoltcp::device::Packet;
use tun_crate::{create_as_async, Configuration, Device, Layer, TunPacket};

#[rd_config]
pub struct TapNetConfig {
    name: Option<String>,
    /// tap address
    tap_addr: String,
    /// ipcidr
    server_addr: String,
    ethernet_addr: Option<String>,
    mtu: usize,
}

pub struct TapServer {
    net: Net,
    name: Option<String>,
    tap_addr: Ipv4Addr,
    server_addr: IpCidr,
    ethernet_addr: Option<EthernetAddress>,
    mtu: usize,
}

#[async_trait]
impl IServer for TapServer {
    async fn start(&self) -> Result<()> {
        let mut config = Configuration::default();
        let netmask = !0u32 >> (32 - self.server_addr.prefix_len());

        config
            .address(IpAddr::from(Ipv4Addr::from(self.tap_addr)))
            .destination(match self.server_addr.address() {
                IpAddress::Ipv4(v4) => IpAddr::from(Ipv4Addr::from(v4)),
                IpAddress::Ipv6(v6) => IpAddr::from(Ipv6Addr::from(v6)),
                _ => unreachable!(),
            })
            .netmask(netmask)
            .layer(Layer::L2)
            .up();

        if let Some(name) = &self.name {
            config.name(name);
        }

        let dev = create_as_async(&config).map_err(map_other)?;
        let device = dev.get_ref().name().to_string();
        let dev = dev
            .into_framed()
            .take_while(|i| ready(i.is_ok()))
            .map(|i| i.unwrap().get_bytes().to_vec())
            .with(|p: Packet| ready(io::Result::Ok(TunPacket::new(p))));

        let server = RawServer::new_device(
            self.net.clone(),
            &device,
            dev,
            self.ethernet_addr,
            self.server_addr,
            128,
            self.mtu,
        )?;

        server.start().await?;

        tracing::error!("Raw server stopped");

        Ok(())
    }
}

impl ServerFactory for TapServer {
    const NAME: &'static str = "tap";

    type Config = TapNetConfig;

    type Server = TapServer;

    fn new(_listen: Net, net: Net, config: Self::Config) -> Result<Self::Server> {
        let tap_addr = Ipv4Addr::from_str(&config.tap_addr)
            .map_err(|_| Error::Other("Failed to parse tap_addr".into()))?;
        let server_addr = IpCidr::from_str(&config.server_addr)
            .map_err(|_| Error::Other("Failed to parse server_addr".into()))?;
        let ethernet_addr = match config.ethernet_addr {
            Some(ethernet_addr) => Some(
                EthernetAddress::from_str(&ethernet_addr)
                    .map_err(|_| Error::Other("Failed to parse ethernet_addr".into()))?,
            ),
            None => None,
        };
        Ok(TapServer {
            net,
            name: config.name,
            tap_addr,
            server_addr,
            ethernet_addr,
            mtu: config.mtu,
        })
    }
}
