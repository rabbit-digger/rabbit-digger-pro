use rd_interface::{constant::UDP_BUFFER_SIZE, Context, Result, UdpChannel, UdpSocket};
use tokio::select;
use tracing::instrument;

#[instrument(err, skip(udp_channel, udp), fields(ctx = ?_ctx))]
pub async fn connect_udp(
    _ctx: &mut Context,
    udp_channel: UdpChannel,
    udp: UdpSocket,
) -> Result<()> {
    let send = async {
        let mut buf = [0u8; UDP_BUFFER_SIZE];
        while let Ok((size, addr)) = udp_channel.recv_send_to(&mut buf).await {
            let buf = &buf[..size];
            udp.send_to(buf, addr).await?;
        }
        Ok(())
    };
    let recv = async {
        let mut buf = [0u8; UDP_BUFFER_SIZE];
        while let Ok((size, addr)) = udp.recv_from(&mut buf).await {
            let buf = &buf[..size];
            udp_channel.send_recv_from(buf, addr).await?;
        }
        Ok(())
    };

    select! {
        r = send => r,
        r = recv => r,
    }
}
