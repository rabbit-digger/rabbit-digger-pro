use pcap::{Capture, Device};
use rd_interface::{Error, ErrorContext, Result};
use tokio_smoltcp::device::Interface;

pub fn get_device_name(name: &str) -> Result<Device> {
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
pub fn get_by_device(device: Device) -> Result<impl Interface> {
    use futures::StreamExt;
    use std::future::ready;
    use std::io;
    use tokio_smoltcp::util::AsyncCapture;

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
    )
    .context("Failed to create async capture")?
    .take_while(|i| ready(i.is_ok()))
    .map(|i| i.unwrap()))
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
