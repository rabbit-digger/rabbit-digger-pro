use std::{future::Future, io, net::SocketAddr, pin::Pin, task::Context, task::Poll};

use futures::ready;
use rd_interface::{Address, ReadBuf, UdpChannel, UdpSocket};
use tracing::instrument;

use super::DropAbort;

struct CopyBidirectional {
    a: UdpChannel,
    b: UdpSocket,
    send_to_buf: Vec<u8>,
    recv_from_buf: Vec<u8>,

    // fill, address
    send_to: Option<(usize, Address)>,
    recv_from: Option<(usize, SocketAddr)>,
}

impl CopyBidirectional {
    fn poll_send_to(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let CopyBidirectional {
            a,
            b,
            send_to,
            send_to_buf,
            ..
        } = self;

        loop {
            if let Some((fill, addr)) = send_to {
                ready!(b.poll_send_to(cx, &send_to_buf[..*fill], addr))?;
                *send_to = None;
            } else {
                let mut buf = ReadBuf::new(send_to_buf);
                let target = ready!(a.poll_send_to(cx, &mut buf))?;
                *send_to = Some((buf.filled().len(), target));
            }
        }
    }
    fn poll_recv_from(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let CopyBidirectional {
            a,
            b,
            recv_from,
            recv_from_buf,
            ..
        } = self;

        loop {
            if let Some((fill, addr)) = recv_from {
                ready!(a.poll_recv_from(cx, &mut recv_from_buf[..*fill], addr))?;
                *recv_from = None;
            } else {
                let mut buf = ReadBuf::new(recv_from_buf);
                let addr = ready!(b.poll_recv_from(cx, &mut buf))?;
                *recv_from = Some((buf.filled().len(), addr));
            }
        }
    }
}

impl Future for CopyBidirectional {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let a_to_b = self.poll_send_to(cx)?;
        let b_to_a = self.poll_recv_from(cx)?;

        match (a_to_b, b_to_a) {
            (Poll::Pending, Poll::Pending) => Poll::Pending,
            _ => Poll::Ready(Ok(())),
        }
    }
}

/// Connect two `TcpStream`. Unlike `copy_bidirectional`, it closes the other side once one side is done.
#[instrument(err, skip(a, b), fields(ctx = ?_ctx))]
pub async fn connect_udp(
    _ctx: &mut rd_interface::Context,
    a: UdpChannel,
    b: UdpSocket,
) -> io::Result<()> {
    DropAbort::new(tokio::spawn(CopyBidirectional {
        a,
        b,
        send_to: None,
        recv_from: None,
        send_to_buf: vec![0; 4096],
        recv_from_buf: vec![0; 4096],
    }))
    .await??;

    Ok(())
}
