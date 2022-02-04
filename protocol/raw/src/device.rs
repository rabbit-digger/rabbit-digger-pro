use std::{net::Ipv4Addr, str::FromStr};

use crate::config::{DeviceConfig, Layer, RawNetConfig, TunTap};
use boxed::BoxedAsyncDevice;
pub use interface_info::get_interface_info;
use pcap::{Capture, Device};
use rd_interface::{Error, ErrorContext, Result};
use tokio_smoltcp::{
    device::AsyncDevice,
    smoltcp::wire::{EthernetAddress, IpCidr},
};

mod boxed;
mod interface_info;
#[cfg(unix)]
mod unix;

pub fn get_device(config: &RawNetConfig) -> Result<(EthernetAddress, BoxedAsyncDevice)> {
    #[cfg(unix)]
    use crate::device::unix::{get_tun, TunTapSetup};
    let (ethernet_address, device) = match &config.device {
        DeviceConfig::String(dev) => {
            let ethernet_address = crate::device::get_interface_info(&dev)
                .context("Failed to get interface info")?
                .ethernet_address;

            let device = Box::new(get_by_device(pcap_device_by_name(&dev)?)?);

            (ethernet_address, BoxedAsyncDevice(device))
        }
        DeviceConfig::Other(cfg) => {
            let host_addr = Ipv4Addr::from_str(&cfg.host_addr)
                .map_err(|_| Error::Other("Failed to parse host_addr".into()))?;
            let destination_addr = IpCidr::from_str(&config.ip_addr)
                .map_err(|_| Error::Other("Failed to parse server_addr".into()))?;

            let setup = TunTapSetup {
                name: cfg.name.clone(),
                addr: host_addr,
                destination_addr,
                mtu: config.mtu,
                layer: match cfg.tuntap {
                    TunTap::Tap => Layer::L2,
                    TunTap::Tun => Layer::L3,
                },
            };
            let device = Box::new(get_tun(setup)?);
            let ethernet_addr = match cfg.tuntap {
                TunTap::Tun => EthernetAddress::BROADCAST,
                TunTap::Tap => {
                    crate::device::get_interface_info(&device.name())
                        .context("Failed to get interface info")?
                        .ethernet_address
                }
            };

            (ethernet_addr, BoxedAsyncDevice(device))
        }
    };

    Ok((ethernet_address, device))
}

fn pcap_device_by_name(name: &str) -> Result<Device> {
    let mut devices = Device::list().context("Failed to list device")?;

    if let Some(id) = devices.iter().position(|d| d.name == name) {
        Ok(devices.remove(id))
    } else {
        Err(Error::Other(
            format!(
                "Failed to find device {} from {:?}",
                name,
                devices
                    .into_iter()
                    .map(|i| format!("[{}] {}", i.name, i.desc.unwrap_or_default()))
                    .collect::<Vec<String>>()
            )
            .into(),
        ))
    }
}

#[cfg(unix)]
pub fn get_by_device(device: Device) -> Result<impl AsyncDevice> {
    use std::io;
    use tokio_smoltcp::{
        device::{AsyncCapture, DeviceCapabilities},
        smoltcp::phy::Checksum,
    };

    let cap = Capture::from_device(device.clone())
        .context("Failed to capture device")?
        .promisc(true)
        .immediate_mode(true)
        .timeout(5)
        .open()
        .context("Failed to open device")?;

    fn map_err(e: pcap::Error) -> io::Error {
        match e {
            pcap::Error::IoError(e) => e.into(),
            pcap::Error::TimeoutExpired => io::ErrorKind::WouldBlock.into(),
            other => io::Error::new(io::ErrorKind::Other, other),
        }
    }
    let mut caps = DeviceCapabilities::default();
    caps.max_transmission_unit = 1500;
    caps.checksum.ipv4 = Checksum::Tx;
    caps.checksum.tcp = Checksum::Tx;
    caps.checksum.udp = Checksum::Tx;
    caps.checksum.icmpv4 = Checksum::Tx;
    caps.checksum.icmpv6 = Checksum::Tx;

    Ok(AsyncCapture::new(
        cap.setnonblock().context("Failed to set nonblock")?,
        |d| {
            let r = d.next().map_err(map_err).map(|p| p.to_vec());
            // eprintln!("recv {:?}", r);
            r
        },
        |d, pkt| {
            let r = d.sendpacket(pkt).map_err(map_err);
            // eprintln!("send {:?}", r);
            r
        },
        caps,
    )
    .context("Failed to create async capture")?)
}

#[cfg(windows)]
pub fn get_by_device(device: Device) -> Result<impl Interface> {
    use tokio::sync::mpsc::{Receiver, Sender};
    use tokio_smoltcp::util::ChannelCapture;

    let mut cap = Capture::from_device(device.clone())
        .context("Failed to capture device")?
        .promisc(true)
        .immediate_mode(true)
        .timeout(5)
        .open()
        .context("Failed to open device")?;
    let mut send = Capture::from_device(device)
        .context("Failed to capture device")?
        .promisc(true)
        .immediate_mode(true)
        .timeout(5)
        .open()
        .context("Failed to open device")?;

    let recv = move |tx: Sender<Vec<u8>>| loop {
        let p = match cap.next().map(|p| p.to_vec()) {
            Ok(p) => p,
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                eprintln!("Error: {:?}", e);
                break;
            }
        };
        tx.blocking_send(p).unwrap();
    };
    let send = move |mut rx: Receiver<Vec<u8>>| {
        while let Some(pkt) = rx.blocking_recv() {
            send.sendpacket(pkt).unwrap();
        }
    };
    let capture = ChannelCapture::new(recv, send);
    Ok(capture)
}
