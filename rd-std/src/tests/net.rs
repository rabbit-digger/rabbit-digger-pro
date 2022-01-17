use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    io::{self, ErrorKind},
    mem::replace,
    net::SocketAddr,
    pin::Pin,
    sync::atomic::{AtomicU16, Ordering},
    task::{self, Poll},
};

use super::channel::Channel;
use futures::{lock::Mutex, ready, Future, Sink, SinkExt, Stream, StreamExt};
use rd_interface::{
    async_trait, Address, Arc, AsyncRead, AsyncWrite, Bytes, BytesMut, Context, Error, INet,
    ITcpListener, ITcpStream, IUdpSocket, IntoDyn, ReadBuf, Result, TcpListener, TcpStream,
    UdpSocket,
};
use tokio::sync::mpsc::error::SendError;

#[derive(Debug)]
struct Inner {
    ports: HashMap<Port, Value>,
    next_port: AtomicU16,
}

/// A test net that can be used for testing.
/// It simulates a network on a localhost, without any real network.
pub struct TestNet {
    inner: Arc<Mutex<Inner>>,
}

impl TestNet {
    pub fn new() -> Self {
        TestNet {
            inner: Arc::new(Mutex::new(Inner {
                ports: HashMap::new(),
                next_port: AtomicU16::new(1),
            })),
        }
    }
}

impl Inner {
    fn next_port(&self, _protocol: Protocol) -> u16 {
        self.next_port.fetch_add(1, Ordering::Relaxed)
    }
    fn get_port(&self, protocol: Protocol, port: u16) -> io::Result<Port> {
        let key = Port(
            protocol,
            if port == 0 {
                self.next_port(Protocol::Udp)
            } else {
                port
            },
        );
        if self.ports.contains_key(&key) {
            return Err(ErrorKind::AddrInUse.into());
        }
        Ok(key)
    }
}

fn make_sa(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn refused() -> Error {
    Error::IO(ErrorKind::ConnectionRefused.into())
}

#[async_trait]
impl INet for TestNet {
    async fn tcp_connect(&self, _ctx: &mut Context, addr: &Address) -> Result<TcpStream> {
        check_address(&addr)?;
        let target_key = Port(Protocol::Tcp, addr.port());

        let mut inner = self.inner.lock().await;
        let key = inner.get_port(Protocol::Tcp, 0)?;

        let (tcp_socket, mut other) = MyTcpStream::new(TcpData {
            is_flushing: false,
            buf: VecDeque::new(),
            local_addr: key,
            peer_addr: target_key,
        });
        other.data.swap();

        match inner.ports.get_mut(&target_key) {
            Some(v) => {
                let listener = v.get_tcp_listener()?;
                listener.channel.send(other).await.map_err(map_err)?;
                Ok(tcp_socket.into_dyn())
            }
            None => Err(refused()),
        }
    }

    async fn tcp_bind(&self, _ctx: &mut Context, addr: &Address) -> Result<TcpListener> {
        check_address(&addr)?;
        let mut inner = self.inner.lock().await;
        let key = inner.get_port(Protocol::Tcp, addr.port())?;
        let (listener, sender) = Pipe::new(key);

        inner.ports.insert(key, Value::TcpListener(sender));
        Ok(MyTcpListener(Mutex::new(listener)).into_dyn())
    }

    async fn udp_bind(&self, _ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        check_address(&addr)?;
        let mut inner = self.inner.lock().await;
        let key = inner.get_port(Protocol::Udp, addr.port())?;
        let (udp_socket, other) = Pipe::new(UdpData {
            inner: self.inner.clone(),
            local_addr: key,
            buf: None,
            flushing: None,
        });
        inner.ports.insert(key, Value::UdpSocket(other));
        Ok(MyUdpSocket(udp_socket).into_dyn())
    }

    async fn lookup_host(&self, addr: &Address) -> Result<Vec<SocketAddr>> {
        Ok(vec![make_sa(addr.port())])
    }
}

#[derive(Debug, Clone)]
pub struct TcpData {
    is_flushing: bool,
    buf: VecDeque<u8>,
    local_addr: Port,
    peer_addr: Port,
}
#[derive(Debug, Clone)]
pub struct UdpData {
    inner: Arc<Mutex<Inner>>,
    local_addr: Port,
    buf: Option<(BytesMut, u16)>,
    flushing: Option<u16>,
}
pub type MyTcpStream = Pipe<Vec<u8>, TcpData>;
#[derive(Debug)]
pub struct MyTcpListener(Mutex<Pipe<MyTcpStream, Port>>);
#[derive(Debug)]
pub struct MyUdpSocket(Pipe<(BytesMut, SocketAddr), UdpData>);

#[derive(Debug, PartialOrd, PartialEq, Ord, Eq, Copy, Clone, Hash)]
enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug)]
enum Value {
    TcpListener(Pipe<MyTcpStream, Port>),
    UdpSocket(Pipe<(BytesMut, SocketAddr), UdpData>),
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Ord, Eq, Hash)]
pub struct Port(Protocol, u16);

impl TcpData {
    fn swap(&mut self) {
        std::mem::swap(&mut self.local_addr, &mut self.peer_addr)
    }
}

impl Into<SocketAddr> for Port {
    fn into(self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.1))
    }
}

#[derive(Debug)]
pub struct Pipe<T: Debug, Data: Clone> {
    channel: Channel<T>,
    data: Data,
}

impl<T: Debug + Send + 'static, Data: Clone> Pipe<T, Data> {
    fn new(data: Data) -> (Self, Self) {
        let (c1, c2) = Channel::new();
        (
            Self {
                channel: c1,
                data: data.clone(),
            },
            Self { channel: c2, data },
        )
    }
}

impl AsyncRead for MyTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let (first, _) = self.data.buf.as_slices();
        if first.len() > 0 {
            let to_copy = first.len().min(buf.remaining());
            buf.initialize_unfilled_to(to_copy)
                .copy_from_slice(&first[0..to_copy]);
            self.data.buf.drain(0..to_copy);
            buf.advance(to_copy);
            Ok(()).into()
        } else {
            let item = { ready!(self.channel.poll_next_unpin(cx)) };
            match item {
                Some(mut data) => {
                    let to_copy = data.len().min(buf.remaining());
                    buf.initialize_unfilled_to(to_copy)
                        .copy_from_slice(&data[..to_copy]);
                    data.drain(0..to_copy);
                    buf.advance(to_copy);
                    self.data.buf.append(&mut data.into());
                    Ok(()).into()
                }
                None => Ok(()).into(),
            }
        }
    }
}
impl AsyncWrite for MyTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let MyTcpStream { channel, data, .. } = &mut *self;

        loop {
            if data.is_flushing {
                ready!(channel.poll_flush_unpin(cx)).map_err(map_err)?;
                data.is_flushing = false;
                return Ok(buf.len()).into();
            }
            ready!(channel.poll_ready_unpin(cx)).map_err(map_err)?;
            channel.start_send_unpin(Vec::from(buf)).map_err(map_err)?;
            data.is_flushing = true;
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.channel.poll_flush_unpin(cx)).map_err(map_err)?;
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.channel.poll_close_unpin(cx)).map_err(map_err)?;
        Poll::Ready(Ok(()))
    }
}

#[async_trait]
impl ITcpStream for MyTcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        Ok(self.data.peer_addr.into())
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.data.local_addr.into())
    }
}

#[async_trait]
impl ITcpListener for MyTcpListener {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        match self.0.lock().await.channel.next().await {
            Some(t) => {
                let addr = t.data.peer_addr.clone().into();
                Ok((t.into_dyn(), addr))
            }
            None => Err(refused()),
        }
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.lock().await.data.into())
    }
}

fn map_err<T>(_e: SendError<T>) -> io::Error {
    ErrorKind::BrokenPipe.into()
}

impl Stream for MyUdpSocket {
    type Item = io::Result<(BytesMut, SocketAddr)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.channel.poll_next_unpin(cx).map(|i| i.map(|j| Ok(j)))
    }
}

impl MyUdpSocket {
    fn poll_udp_port<F, R>(inner: &mut Inner, port: u16, func: F, default: R) -> R
    where
        F: FnOnce(&mut Pipe<(BytesMut, SocketAddr), UdpData>) -> R,
    {
        let target_key = Port(Protocol::Udp, port);
        match inner.ports.get_mut(&target_key) {
            Some(v) => match v.get_udp_socket() {
                Ok(v) => func(v),
                Err(_) => default,
            },
            None => default,
        }
    }
    fn poll_send_to(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        let UdpData {
            inner,
            buf,
            flushing,
            local_addr,
            ..
        } = &mut self.0.data;
        let mut inner = ready!(Pin::new(&mut inner.lock()).poll(cx));

        loop {
            if let Some(port) = flushing {
                ready!(Self::poll_udp_port(
                    &mut inner,
                    *port,
                    |u| u.channel.poll_flush_unpin(cx),
                    Poll::Ready(Ok(()))
                ))
                .map_err(map_err)?;
                *flushing = None;
                break;
            }
            if let Some(item) = buf {
                let addr = item.1.clone();
                ready!(Self::poll_udp_port(
                    &mut inner,
                    addr,
                    |u| u.channel.poll_ready_unpin(cx),
                    Poll::Ready(Ok(()))
                ))
                .map_err(map_err)?;

                let (b, a) = replace(buf, None).unwrap();
                let from_addr: SocketAddr = local_addr.clone().into();
                Self::poll_udp_port(
                    &mut inner,
                    addr,
                    |u| u.channel.start_send_unpin((b, from_addr)),
                    Ok(()),
                )
                .map_err(map_err)?;

                *flushing = Some(a);
            } else {
                break;
            }
        }

        Poll::Ready(Ok(()))
    }
}

impl Sink<(Bytes, Address)> for MyUdpSocket {
    type Error = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.poll_send_to(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        (buf, addr): (Bytes, Address),
    ) -> Result<(), Self::Error> {
        self.0.data.buf = Some((BytesMut::from(&buf[..]), addr.port()));
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.poll_send_to(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.0.channel.poll_close_unpin(cx).map_err(map_err)
    }
}

#[async_trait]
impl IUdpSocket for MyUdpSocket {
    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.0.data.local_addr.into())
    }
}

fn check_address(addr: &Address) -> io::Result<()> {
    let addr = addr.to_socket_addr()?;
    if addr.ip().is_loopback() {
        Ok(())
    } else if addr.ip().is_unspecified() {
        Ok(())
    } else {
        Err(ErrorKind::AddrNotAvailable.into())
    }
}

impl Value {
    fn get_tcp_listener(&mut self) -> io::Result<&mut Pipe<MyTcpStream, Port>> {
        match self {
            Value::TcpListener(s) => Ok(s),
            _ => Err(ErrorKind::ConnectionRefused.into()),
        }
    }
    fn get_udp_socket(&mut self) -> io::Result<&mut Pipe<(BytesMut, SocketAddr), UdpData>> {
        match self {
            Value::UdpSocket(s) => Ok(s),
            _ => Err(ErrorKind::ConnectionRefused.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::tests::{assert_echo, assert_echo_udp, spawn_echo_server, spawn_echo_server_udp};

    use super::*;
    use rd_interface::Context;

    #[tokio::test]
    async fn test_tcp() {
        let net = TestNet::new().into_dyn();
        let addr = Address::from_str("127.0.0.1:1234").unwrap();
        spawn_echo_server(&net, &addr).await;
        assert_echo(&net, addr).await;
    }

    #[tokio::test]
    async fn test_tcp_listener_local_addr() {
        let net = TestNet::new();
        let addr = Address::from_str("127.0.0.1:12345").unwrap();
        let socket = net.tcp_bind(&mut Context::new(), &addr).await.unwrap();

        assert_eq!(
            socket.local_addr().await.unwrap(),
            addr.to_socket_addr().unwrap()
        );

        let socket = net
            .tcp_bind(&mut Context::new(), &"0.0.0.0:0".parse().unwrap())
            .await
            .unwrap();
        assert_eq!(
            socket.local_addr().await.unwrap(),
            "127.0.0.1:1".parse().unwrap()
        );
    }

    #[tokio::test]
    async fn test_tcp_stream_addr() {
        let net = TestNet::new();
        let addr = "127.0.0.1:12345".parse().unwrap();
        let server = net.tcp_bind(&mut Context::new(), &addr).await.unwrap();

        let socket = net.tcp_connect(&mut Context::new(), &addr).await.unwrap();
        let (accepted, accepted_addr) = server.accept().await.unwrap();

        assert_eq!(
            socket.peer_addr().await.unwrap(),
            addr.to_socket_addr().unwrap()
        );
        assert_eq!(
            socket.local_addr().await.unwrap(),
            "127.0.0.1:1".parse().unwrap()
        );

        assert_eq!(
            accepted.local_addr().await.unwrap(),
            addr.to_socket_addr().unwrap()
        );
        assert_eq!(
            accepted.peer_addr().await.unwrap(),
            "127.0.0.1:1".parse().unwrap()
        );
        assert_eq!(accepted_addr, "127.0.0.1:1".parse().unwrap())
    }

    #[tokio::test]
    async fn test_udp() {
        let addr = Address::from_str("127.0.0.1:1234").unwrap();

        let net = TestNet::new().into_dyn();
        spawn_echo_server_udp(&net, &addr).await;
        assert_echo_udp(&net, &addr).await;
    }
}
