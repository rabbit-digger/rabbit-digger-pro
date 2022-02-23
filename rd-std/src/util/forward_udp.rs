use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
    time::Duration,
};

use self::connection::UdpConnection;
use crate::util::LruCache;
use futures::{ready, Future};
use rd_interface::{constant::UDP_BUFFER_SIZE, Address, Net, ReadBuf};
use tokio::sync::mpsc::{channel, Receiver, Sender};

mod connection;
mod send_back;

const TIME_TO_LIVE: Duration = Duration::from_secs(30);

pub struct UdpEndpoint {
    pub from: SocketAddr,
    pub to: SocketAddr,
}

#[derive(Debug)]
pub struct UdpPacket {
    pub from: SocketAddr,
    pub to: SocketAddr,
    pub data: Vec<u8>,
}

impl UdpPacket {
    pub fn new(data: Vec<u8>, from: SocketAddr, to: SocketAddr) -> Self {
        UdpPacket { from, to, data }
    }
}

// Stream: (data, from, to)
// Sink: (data, to, from)
pub trait RawUdpSource: Unpin + Send + Sync {
    fn poll_recv(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<UdpEndpoint>>;
    fn poll_send(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &[u8],
        endpoint: &UdpEndpoint,
    ) -> Poll<io::Result<()>>;
}

struct ForwardUdp<S> {
    s: S,
    net: Net,
    conn: LruCache<SocketAddr, UdpConnection>,
    send_back: Sender<UdpPacket>,
    recv_back: Receiver<UdpPacket>,
    channel_size: usize,
    recv_buf: Vec<u8>,
    send_buf: Option<UdpPacket>,
}

impl<S> ForwardUdp<S>
where
    S: RawUdpSource,
{
    fn new(s: S, net: Net, channel_size: usize) -> Self {
        let (tx, rx) = channel(channel_size);

        ForwardUdp {
            s,
            net,
            conn: LruCache::with_expiry_duration_and_capacity(TIME_TO_LIVE, 256),
            send_back: tx,
            recv_back: rx,
            channel_size,
            recv_buf: vec![0; UDP_BUFFER_SIZE],
            send_buf: None,
        }
    }
}

impl<S> ForwardUdp<S>
where
    S: RawUdpSource,
{
    fn get(&mut self, bind_from: SocketAddr) -> &mut UdpConnection {
        let net = &self.net;
        let send_back = self.send_back.clone();
        let channel_size = self.channel_size;
        self.conn.entry(bind_from).or_insert_with(|| {
            let net = net.clone();
            let bind_addr = Address::any_addr_port(&bind_from);

            UdpConnection::new(net, bind_from, bind_addr, send_back, channel_size)
        })
    }
    fn poll_recv_packet(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        loop {
            let mut buf = ReadBuf::new(&mut self.recv_buf);
            let item = ready!(self.s.poll_recv(cx, &mut buf))?;
            let buf = buf.filled().to_vec();

            let UdpEndpoint { from, to } = item;
            let udp = self.get(from);
            if let Err(e) = udp.send((buf, to)) {
                tracing::warn!("udp send buffer full. {}", e);
            }
        }
    }
    fn poll_send_back(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match &self.send_buf {
                Some(UdpPacket { data, from, to }) => {
                    ready!(self.s.poll_send(
                        cx,
                        &data,
                        &UdpEndpoint {
                            from: *from,
                            to: *to
                        }
                    ))?;
                    self.send_buf = None;
                }
                None => {
                    let packet = match self.recv_back.poll_recv(cx) {
                        Poll::Ready(Some(result)) => result,
                        Poll::Ready(None) => return Poll::Ready(Ok(())),
                        Poll::Pending => return Poll::Pending,
                    };
                    self.send_buf = Some(packet);
                }
            }
        }
    }
}

impl<S> Future for ForwardUdp<S>
where
    S: RawUdpSource,
{
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let a_to_b = self.poll_recv_packet(cx)?;
        let b_to_a = self.poll_send_back(cx)?;
        self.conn.poll_clear_expired(cx);

        match (a_to_b, b_to_a) {
            (Poll::Pending, Poll::Pending) => Poll::Pending,
            _ => Poll::Ready(Ok(())),
        }
    }
}

pub async fn forward_udp<S>(s: S, net: Net, channel_size: Option<usize>) -> io::Result<()>
where
    S: RawUdpSource,
{
    ForwardUdp::new(s, net, channel_size.unwrap_or(128)).await
}
