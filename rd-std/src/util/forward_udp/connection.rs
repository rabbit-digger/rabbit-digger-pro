use std::{io, net::SocketAddr};

use crate::util::connect_udp;

use super::{send_back::BackChannel, UdpPacket};
use rd_interface::{Address, Bytes, Context, IntoDyn, Net, Result};
use tokio::{
    sync::mpsc::{channel, Sender},
    task::JoinHandle,
};

pub(super) struct UdpConnection {
    handle: JoinHandle<Result<()>>,
    send_udp: Sender<(Bytes, SocketAddr)>,
}

impl UdpConnection {
    pub(super) fn new(
        net: Net,
        bind_from: SocketAddr,
        send_back: Sender<UdpPacket>,
        channel_size: usize,
    ) -> UdpConnection {
        let (send_udp, rx) = channel(channel_size);
        let back_channel = BackChannel::new(bind_from, send_back, rx).into_dyn();
        let fut = async move {
            let bind_addr: Address = bind_from.into();
            let mut ctx = Context::from_socketaddr(bind_from);
            let udp = net
                .udp_bind(&mut ctx, &bind_addr.to_any_addr_port()?)
                .await?;

            connect_udp(&mut ctx, back_channel, udp).await?;

            Ok(())
        };

        UdpConnection {
            handle: tokio::spawn(fut),
            send_udp,
        }
    }
    pub(super) fn send(&mut self, packet: (Bytes, SocketAddr)) -> Result<()> {
        self.send_udp
            .try_send(packet)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e).into())
    }
}

impl Drop for UdpConnection {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
