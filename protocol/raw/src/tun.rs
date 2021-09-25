use std::{
    future::ready,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

use crate::server::{Layer, RawServer};
use futures::{SinkExt, StreamExt};
use rd_interface::{
    async_trait, error::map_other, prelude::*, registry::ServerFactory, Error, IServer, Net, Result,
};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};
use tokio_smoltcp::device::Packet;
use tun_crate::{create_as_async, Configuration, Device, TunPacket};

#[rd_config]
pub struct TunNetConfig {
    dev_name: Option<String>,
    /// tun address
    tun_addr: String,
    /// ipcidr
    server_addr: String,
    ethernet_addr: Option<String>,
    mtu: usize,
}

pub struct TunServer {
    net: Net,
    name: Option<String>,
    tun_addr: Ipv4Addr,
    server_addr: IpCidr,
    ethernet_addr: Option<EthernetAddress>,
    mtu: usize,
}

#[async_trait]
impl IServer for TunServer {
    async fn start(&self) -> Result<()> {
        let mut config = Configuration::default();
        let netmask = !0u32 >> (32 - self.server_addr.prefix_len());

        config
            .address(IpAddr::from(Ipv4Addr::from(self.tun_addr)))
            .destination(match self.server_addr.address() {
                IpAddress::Ipv4(v4) => IpAddr::from(Ipv4Addr::from(v4)),
                IpAddress::Ipv6(v6) => IpAddr::from(Ipv6Addr::from(v6)),
                _ => unreachable!(),
            })
            .netmask(netmask)
            .layer(tun_crate::Layer::L3)
            .up();

        if let Some(name) = &self.name {
            config.name(name);
        }

        let dev = create_as_async(&config).map_err(map_other)?;
        let device = dev.get_ref().name().to_string();
        let dev = workaround::PatchAsyncDevice(dev)
            .into_framed()
            .take_while(|i| ready(i.is_ok()))
            .map(|i| i.unwrap().get_bytes().to_vec())
            .with(|p: Packet| ready(io::Result::Ok(TunPacket::new(p))));

        tracing::info!("tun created: {}", device);

        let server = RawServer::new_device(
            self.net.clone(),
            &device,
            dev,
            self.ethernet_addr,
            self.server_addr,
            128,
            self.mtu,
            Layer::L3,
        )?;

        server.start().await?;

        tracing::error!("Raw server stopped");

        Ok(())
    }
}

impl ServerFactory for TunServer {
    const NAME: &'static str = "tun";

    type Config = TunNetConfig;

    type Server = TunServer;

    fn new(_listen: Net, net: Net, config: Self::Config) -> Result<Self::Server> {
        let tun_addr = Ipv4Addr::from_str(&config.tun_addr)
            .map_err(|_| Error::Other("Failed to parse tun_addr".into()))?;
        let server_addr = IpCidr::from_str(&config.server_addr)
            .map_err(|_| Error::Other("Failed to parse server_addr".into()))?;
        let ethernet_addr = match config.ethernet_addr {
            Some(ethernet_addr) => Some(
                EthernetAddress::from_str(&ethernet_addr)
                    .map_err(|_| Error::Other("Failed to parse ethernet_addr".into()))?,
            ),
            None => None,
        };
        Ok(TunServer {
            net,
            name: config.dev_name,
            tun_addr,
            server_addr,
            ethernet_addr,
            mtu: config.mtu,
        })
    }
}

mod workaround {
    use std::{
        io,
        pin::Pin,
        task::{Context, Poll},
    };

    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
    use tokio_util::codec::Framed;
    use tun_crate::{r#async::AsyncDevice, Device, TunPacketCodec};

    pub struct PatchAsyncDevice(pub AsyncDevice);

    impl PatchAsyncDevice {
        pub fn into_framed(mut self) -> Framed<Self, TunPacketCodec> {
            let pi = self.0.get_mut().has_packet_information();
            let codec = TunPacketCodec::new(pi, self.0.get_ref().mtu().unwrap_or(1504));
            Framed::new(self, codec)
        }
    }

    impl AsyncRead for PatchAsyncDevice {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.0).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for PatchAsyncDevice {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.0).poll_write(cx, buf)
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.0).poll_flush(cx)
        }

        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.0).poll_shutdown(cx)
        }
    }
}
