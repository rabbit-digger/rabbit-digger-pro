use crate::traits::{self, async_trait, ProxyTcpListener, ProxyTcpStream};
use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver as Receiver, UnboundedSender as Sender},
    lock::Mutex,
    sink::SinkExt,
    stream::StreamExt,
    AsyncRead, AsyncWrite,
};
use std::{
    collections::BTreeMap,
    io::{ErrorKind, Result},
    net::{Ipv4Addr, Shutdown, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use traits::ProxyUdpSocket;

pub type TcpStream = Pipe<Vec<u8>, SocketAddr>;
pub type TcpListener = Pipe<TcpStream, SocketAddr>;
pub type UdpSocket = Pipe<(Vec<u8>, SocketAddr), SocketAddr>;

#[derive(Debug, PartialOrd, PartialEq, Ord, Eq, Copy, Clone)]
enum Protocol {
    Tcp,
    Udp,
}
enum Value {
    TcpStream(TcpStream),
    TcpListener(TcpListener),
    UdpSocket(UdpSocket),
}
type Port = (Protocol, u16);

struct Inner {
    ports: BTreeMap<Port, Value>,
    next_port: u16,
}

#[derive(Clone)]
pub struct VirtualHost {
    inner: Arc<Mutex<Inner>>,
}

impl VirtualHost {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                ports: BTreeMap::new(),
                next_port: 1,
            })),
        }
    }
}

impl Inner {
    fn next_port(&mut self, protocol: Protocol) -> u16 {
        while self.ports.contains_key(&(protocol, self.next_port)) {
            self.next_port += 1;
        }
        self.next_port
    }
    fn get_port(&mut self, protocol: Protocol, port: u16) -> Result<Port> {
        let key = (
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

pub struct Pipe<T, Meta> {
    sender: Mutex<Sender<T>>,
    receiver: Mutex<Receiver<T>>,
    data: Option<Meta>,
}

impl<T, Meta> Pipe<T, Meta> {
    fn new() -> (Self, Self) {
        let (tx1, rx1) = unbounded();
        let (tx2, rx2) = unbounded();
        (
            Self {
                sender: Mutex::new(tx1),
                receiver: Mutex::new(rx2),
                data: None,
            },
            Self {
                sender: Mutex::new(tx2),
                receiver: Mutex::new(rx1),
                data: None,
            },
        )
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        todo!()
    }
}
impl AsyncWrite for TcpStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        todo!()
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        todo!()
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        todo!()
    }
}

#[async_trait]
impl traits::TcpStream for TcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        todo!()
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        todo!()
    }
    async fn shutdown(&self, how: Shutdown) -> Result<()> {
        todo!()
    }
}

#[async_trait]
impl traits::TcpListener<TcpStream> for TcpListener {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        todo!()
    }
}

#[async_trait]
impl traits::UdpSocket for UdpSocket {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        match self.receiver.lock().await.next().await {
            Some(((dat, addr))) => {
                let to_copy = buf.len().min(dat.len());
                buf.clone_from_slice(&dat[0..to_copy]);
                Ok((to_copy, addr))
            }
            None => Err(ErrorKind::BrokenPipe.into()),
        }
    }
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        let mut sender = self.sender.lock().await;
        match sender.send((Vec::from(buf), addr)).await {
            Ok(_) => Ok(buf.len()),
            Err(_) => Err(ErrorKind::BrokenPipe.into()),
        }
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        todo!()
    }
}

#[async_trait]
impl ProxyTcpListener for VirtualHost {
    type TcpStream = TcpStream;
    type TcpListener = TcpListener;

    async fn tcp_bind(&self, addr: SocketAddr) -> Result<Self::TcpListener> {
        check_address(&addr)?;
        let mut inner = self.inner.lock().await;
        let key = inner.get_port(Protocol::Udp, addr.port())?;
        let (listener, sender) = TcpListener::new();
        inner.ports.insert(key, Value::TcpListener(sender));
        Ok(listener)
    }
}

#[async_trait]
impl ProxyTcpStream for VirtualHost {
    type TcpStream = TcpStream;

    async fn tcp_connect(&self, addr: SocketAddr) -> Result<Self::TcpStream> {
        check_address(&addr)?;
        let mut inner = self.inner.lock().await;
        let key = (Protocol::Tcp, addr.port());
        match inner.ports.get_mut(&key) {
            Some(v) => {
                let sender = v.get_tcp_listener()?;
                let (tcp_socket, other) = TcpStream::new();
                sender.sender.lock().await.send(other).await;
                Ok(tcp_socket)
            }
            None => Err(ErrorKind::ConnectionRefused.into()),
        }
    }
}

#[async_trait]
impl ProxyUdpSocket for VirtualHost {
    type UdpSocket = UdpSocket;

    async fn udp_bind(&self, addr: SocketAddr) -> Result<Self::UdpSocket> {
        check_address(&addr)?;
        let mut inner = self.inner.lock().await;
        let key = inner.get_port(Protocol::Udp, addr.port())?;
        let (udp_socket, other) = UdpSocket::new();
        inner.ports.insert(key, Value::UdpSocket(other));
        Ok(udp_socket)
    }
}

fn check_address(addr: &SocketAddr) -> Result<()> {
    if addr.ip() == Ipv4Addr::new(127, 0, 0, 1) {
        Ok(())
    } else {
        Err(ErrorKind::AddrNotAvailable.into())
    }
}

impl Value {
    fn get_tcp_stream(&mut self) -> Result<&mut TcpStream> {
        match self {
            Value::TcpStream(s) => Ok(s),
            _ => Err(ErrorKind::ConnectionRefused.into()),
        }
    }
    fn get_tcp_listener(&mut self) -> Result<&mut TcpListener> {
        match self {
            Value::TcpListener(s) => Ok(s),
            _ => Err(ErrorKind::ConnectionRefused.into()),
        }
    }
    fn get_udp_socket(&mut self) -> Result<&mut UdpSocket> {
        match self {
            Value::UdpSocket(s) => Ok(s),
            _ => Err(ErrorKind::ConnectionRefused.into()),
        }
    }
}
