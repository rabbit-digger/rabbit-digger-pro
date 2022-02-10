use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
    time::Duration,
};

use self::connection::UdpConnection;
use crate::util::LruCache;
use futures::{ready, Future, Sink, SinkExt, Stream, StreamExt};
use rd_interface::{Address, Bytes, Net};
use tokio::sync::mpsc::{channel, Receiver, Sender};

mod connection;
mod send_back;

const TIME_TO_LIVE: Duration = Duration::from_secs(30);

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

enum SendState {
    WaitReady,
    Recv,
    Flush,
}

struct ForwardUdp<S> {
    s: S,
    net: Net,
    conn: LruCache<SocketAddr, UdpConnection>,
    send_back: Sender<UdpPacket>,
    recv_back: Receiver<UdpPacket>,
    state: SendState,
    need_flush: bool,
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
            conn: LruCache::with_expiry_duration_and_capacity(TIME_TO_LIVE, 256),
            send_back: tx,
            recv_back: rx,
            state: SendState::WaitReady,
            need_flush: false,
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
            let bind_addr = Address::any_addr_port(&bind_from);

            UdpConnection::new(net, bind_from, bind_addr, send_back, channel_size)
        })
    }
    fn poll_recv_packet(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        loop {
            let item = match ready!(self.s.poll_next_unpin(cx)) {
                Some(result) => result?,
                None => return Poll::Ready(Ok(())),
            };

            let UdpPacket { data, from, to } = item;
            let udp = self.get(from);
            if let Err(e) = udp.send((data, to)) {
                tracing::warn!("udp send buffer full. {:?}", e);
            }
        }
    }
    fn poll_send_back(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match self.state {
                SendState::WaitReady => match self.s.poll_ready_unpin(cx)? {
                    Poll::Ready(()) => self.state = SendState::Recv,
                    Poll::Pending => {
                        if self.need_flush {
                            self.state = SendState::Flush;
                            continue;
                        } else {
                            return Poll::Pending;
                        }
                    }
                },
                SendState::Recv => {
                    let packet = match self.recv_back.poll_recv(cx) {
                        Poll::Ready(Some(result)) => result,
                        Poll::Ready(None) => return Poll::Ready(Ok(())),
                        Poll::Pending => {
                            if self.need_flush {
                                self.state = SendState::Flush;
                                continue;
                            } else {
                                return Poll::Pending;
                            }
                        }
                    };
                    self.s.start_send_unpin(packet)?;
                    self.need_flush = true;

                    self.state = SendState::WaitReady;
                }
                SendState::Flush => {
                    ready!(self.s.poll_flush_unpin(cx)?);
                    self.need_flush = false;

                    self.state = SendState::WaitReady;
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
