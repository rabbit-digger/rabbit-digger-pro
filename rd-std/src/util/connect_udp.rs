use futures::{SinkExt, StreamExt};
use rd_interface::{Context, UdpChannel, UdpSocket};
use std::io::Result;
use tokio::select;
use tracing::instrument;

use super::DropAbort;

#[instrument(err, skip(udp_channel, udp), fields(ctx = ?_ctx))]
pub async fn connect_udp(
    _ctx: &mut Context,
    udp_channel: UdpChannel,
    udp: UdpSocket,
) -> Result<()> {
    let (tx1, rx1) = udp_channel.split();
    let (tx2, rx2) = udp.split();

    let mut send = DropAbort::new(tokio::spawn(rx1.forward(tx2.buffer(128))));
    let mut recv = DropAbort::new(tokio::spawn(rx2.forward(tx1.buffer(128))));

    let result = select! {
        r = &mut send => r,
        r = &mut recv => r,
    };
    result??;

    Ok(())
}
