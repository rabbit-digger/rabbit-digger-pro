use std::{io, net::Ipv4Addr, str::FromStr};

use crate::config::{DeviceConfig, RawNetConfig, TunTap, TunTapSetup};
use boxed::BoxedAsyncDevice;
pub use interface_info::get_interface_info;
use pcap::{Capture, Device};
use rd_interface::{Error, ErrorContext, Result};
use tokio_smoltcp::{
    device::{AsyncDevice, DeviceCapabilities},
    smoltcp::{
        phy::Checksum,
        wire::{EthernetAddress, IpCidr},
    },
};

mod boxed;
mod interface_info;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
use crate::device::unix::get_tun;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use crate::device::windows::get_tun;

pub fn get_device(config: &RawNetConfig) -> Result<(EthernetAddress, BoxedAsyncDevice)> {
    let destination_addr = IpCidr::from_str(&config.ip_addr)
        .map_err(|_| Error::Other("Failed to parse server_addr".into()))?;

    let (ethernet_address, device) = match &config.device {
        DeviceConfig::String(dev) => {
            let ethernet_address = crate::device::get_interface_info(&dev)
                .context("Failed to get interface info")?
                .ethernet_address;

            let device = Box::new(get_by_device(
                pcap_device_by_name(&dev)?,
                get_filter(&destination_addr),
            )?);

            (ethernet_address, BoxedAsyncDevice(device))
        }
        DeviceConfig::Other(cfg) => {
            let host_addr = Ipv4Addr::from_str(&cfg.host_addr)
                .map_err(|_| Error::Other("Failed to parse host_addr".into()))?;

            let setup = TunTapSetup {
                name: cfg.name.clone(),
                addr: host_addr,
                destination_addr,
                mtu: config.mtu,
                layer: cfg.tuntap.into(),
            };
            let device = Box::new(get_tun(setup)?);
            let ethernet_addr = match cfg.tuntap {
                TunTap::Tun => EthernetAddress::BROADCAST,
                #[cfg(unix)]
                TunTap::Tap => {
                    crate::device::get_interface_info(&device.name())
                        .context("Failed to get interface info")?
                        .ethernet_address
                }
                #[cfg(windows)]
                TunTap::Tap => unreachable!(),
            };

            (ethernet_addr, BoxedAsyncDevice(device))
        }
    };

    Ok((ethernet_address, device))
}

fn get_filter(ip_cidr: &IpCidr) -> Option<String> {
    match ip_cidr {
        IpCidr::Ipv4(v4) => Some(format!("net {}", v4.network())),
        _ => None,
    }
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
fn get_by_device(device: Device, filter: Option<String>) -> Result<impl AsyncDevice> {
    use tokio_smoltcp::device::AsyncCapture;

    let mut cap = Capture::from_device(device.clone())
        .context("Failed to capture device")?
        .promisc(true)
        .immediate_mode(true)
        .timeout(5)
        .open()
        .context("Failed to open device")?;

    if let Some(filter) = filter {
        cap.filter(&filter, true).context("Failed to add filter")?;
    }

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
fn get_by_device(device: Device, filter: Option<String>) -> Result<impl AsyncDevice> {
    use tokio::sync::mpsc::{Receiver, Sender};
    use tokio_smoltcp::device::ChannelCapture;

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

    if let Some(filter) = filter {
        cap.filter(&filter, true).context("Failed to add filter")?;
    }
    // don't accept any packets from the send device
    send.filter("less 0", true)
        .context("Failed to add filter")?;

    let recv = move |tx: Sender<io::Result<Vec<u8>>>| loop {
        let p = match cap.next().map(|p| p.to_vec()) {
            Ok(p) => p,
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                eprintln!("Error: {:?}", e);
                break;
            }
        };

        tx.blocking_send(Ok(p)).unwrap();
    };
    let send = move |mut rx: Receiver<Vec<u8>>| {
        while let Some(pkt) = rx.blocking_recv() {
            send.sendpacket(pkt).unwrap();
        }
    };
    let mut caps = DeviceCapabilities::default();
    caps.max_transmission_unit = 1500;
    caps.checksum.ipv4 = Checksum::Tx;
    caps.checksum.tcp = Checksum::Tx;
    caps.checksum.udp = Checksum::Tx;
    caps.checksum.icmpv4 = Checksum::Tx;
    caps.checksum.icmpv6 = Checksum::Tx;

    let capture = ChannelCapture::new(recv, send, caps);
    Ok(capture)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_smoltcp::smoltcp::wire::Ipv4Address;

    #[test]
    fn test_get_filter() {
        let filter = get_filter(&IpCidr::new(Ipv4Address::new(192, 168, 1, 1).into(), 24));

        assert_eq!(filter, Some("net 192.168.1.0/24".to_string()));
    }
}
