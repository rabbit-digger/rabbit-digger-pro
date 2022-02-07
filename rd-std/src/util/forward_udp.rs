use std::{
    io,
    mem::replace,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
    time::Duration,
};

use self::connection::UdpConnection;
use futures::{ready, Future, Sink, SinkExt, Stream, StreamExt};
use lru_time_cache::LruCache;
use rd_interface::{Bytes, Net};
use tokio::sync::mpsc::{channel, Receiver, Sender};

mod connection;
mod send_back;

#[derive(Debug)]
pub struct UdpPacket {
    pub from: SocketAddr,
    pub to: SocketAddr,
    pub data: Bytes,
}

impl UdpPacket {
    pub fn new(data: Bytes, from: SocketAddr, to: SocketAddr) -> Self {
        UdpPacket { from, to, data }
    }
}

// Stream: (data, from, to)
// Sink: (data, to, from)
pub trait RawUdpSource:
    Stream<Item = io::Result<UdpPacket>> + Sink<UdpPacket, Error = io::Error> + Unpin + Send + Sync
{
}

struct ForwardUdp<S> {
    s: S,
    net: Net,
    conn: LruCache<SocketAddr, UdpConnection>,
    send_back: Sender<UdpPacket>,
    recv_back: Receiver<UdpPacket>,
    buf: Option<UdpPacket>,
    is_flushing: bool,
    channel_size: usize,
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
            conn: LruCache::with_expiry_duration_and_capacity(Duration::from_secs(30), 256),
            send_back: tx,
            recv_back: rx,
            buf: None,
            is_flushing: false,
            channel_size,
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
            UdpConnection::new(net, bind_from, send_back, channel_size)
        })
    }
    fn poll_recv_packet(&mut self, cx: &mut task::Context<'_>) -> task::Poll<io::Result<()>> {
        loop {
            let item = match ready!(self.s.poll_next_unpin(cx)) {
                Some(result) => result?,
                None => return task::Poll::Ready(Ok(())),
            };

            let UdpPacket { data, from, to } = item;
            let udp = self.get(from);
            if let Err(e) = udp.send((data, to)) {
                tracing::warn!("udp send buffer full. {:?}", e);
            }
        }
    }
    fn poll_send_back(&mut self, cx: &mut task::Context<'_>) -> task::Poll<io::Result<()>> {
        loop {
            if self.is_flushing {
                ready!(self.s.poll_flush_unpin(cx)?);
                self.is_flushing = false;
            }
            match &self.buf {
                Some(_) => {
                    ready!(self.s.poll_ready_unpin(cx))?;
                    let packet = replace(&mut self.buf, None).unwrap();
                    self.s.start_send_unpin(packet)?;
                    self.is_flushing = true;
                }
                None => {
                    let packet = match ready!(self.recv_back.poll_recv(cx)) {
                        Some(result) => result,
                        None => return Poll::Ready(Ok(())),
                    };
                    self.buf = Some(packet);
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

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let a_to_b = self.poll_recv_packet(cx)?;
        let b_to_a = self.poll_send_back(cx)?;

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
    ForwardUdp::new(s, net, channel_size.unwrap_or(1024)).await
}
