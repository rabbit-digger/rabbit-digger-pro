use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

use crate::server::{RawServer, RawServerConfig};
use futures::StreamExt;
use rd_interface::{
    async_trait, error::map_other, prelude::*, registry::ServerFactory, Error, IServer, Net, Result,
};
use smoltcp::wire::{IpAddress, IpCidr};
use tun_crate::{create_as_async, Configuration, Device, Layer};

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
    ethernet_addr: Option<String>,
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

        #[cfg(target_os = "linux")]
        config.platform(|config| {
            config.packet_information(true);
        });

        if let Some(name) = &self.name {
            config.name(name);
        }

        let mut dev = create_as_async(&config).map_err(map_other)?.into_framed();
        let inner_dev = dev.get_ref().get_ref();

        let server = RawServer::new(
            self.net.clone(),
            RawServerConfig {
                device: inner_dev.name().into(),
                mtu: self.mtu,
                ip_addr: self.server_addr.to_string(),
                ethernet_addr: self.ethernet_addr.clone(),
                lru_size: 128,
            },
        )?;

        server.start().await?;

        tracing::error!("Raw server stopped");

        while let Some(p) = dev.next().await {
            println!("{:?}", p)
        }

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
        Ok(TapServer {
            net,
            name: config.name,
            tap_addr,
            server_addr,
            ethernet_addr: config.ethernet_addr,
            mtu: config.mtu,
        })
    }
}
