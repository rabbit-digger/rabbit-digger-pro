//! Virtual host (WIP)

#![allow(dead_code, unused_variables)]

use crate::traits::{self, async_trait, ProxyTcpListener, ProxyTcpStream, Runtime};
use futures::{
    channel::mpsc::{
        unbounded, SendError, UnboundedReceiver as Receiver, UnboundedSender as Sender,
    },
    lock::Mutex,
    ready,
    sink::SinkExt,
    stream::StreamExt,
    AsyncRead, AsyncWrite,
};
use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    io::{Error, ErrorKind, Result},
    net::{Ipv4Addr, Shutdown, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use traits::{ProxyResolver, ProxyUdpSocket};

#[derive(Debug, Clone)]
pub struct TcpData {
    buf: VecDeque<u8>,
    local_addr: Port,
    peer_addr: Port,
}
#[derive(Debug, Clone)]
pub struct UdpData {
    inner: Arc<Mutex<Inner>>,
    local_addr: Port,
}
pub type TcpStream = Pipe<Vec<u8>, TcpData>;
pub type TcpListener = Pipe<TcpStream, Port>;
pub type UdpSocket = Pipe<(Vec<u8>, SocketAddr), UdpData>;

#[derive(Debug, PartialOrd, PartialEq, Ord, Eq, Copy, Clone)]
enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug)]
enum Value {
    TcpStream(TcpStream),
    TcpListener(TcpListener),
    UdpSocket(UdpSocket),
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Ord, Eq)]
pub struct Port(Protocol, u16);

impl TcpData {
    fn swap(&mut self) {
        std::mem::swap(&mut self.local_addr, &mut self.peer_addr)
    }
}

impl Into<SocketAddr> for Port {
    fn into(self) -> SocketAddr {
        SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), self.1)
    }
}

#[derive(Debug)]
struct Inner {
    ports: BTreeMap<Port, Value>,
    next_port: u16,
}

#[derive(Debug)]
pub struct VirtualHost<PR = ()>
where
    PR: Sized,
{
    inner: Arc<Mutex<Inner>>,
    pr: Arc<Option<PR>>,
}

impl<PR> Clone for VirtualHost<PR> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            pr: self.pr.clone(),
        }
    }
}

impl VirtualHost<()> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                ports: BTreeMap::new(),
                next_port: 1,
            })),
            pr: Arc::new(None),
        }
    }
}

impl<PR> VirtualHost<PR> {
    pub fn with_pr(pr: PR) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                ports: BTreeMap::new(),
                next_port: 1,
            })),
            pr: Arc::new(Some(pr)),
        }
    }
}

impl Inner {
    fn next_port(&mut self, protocol: Protocol) -> u16 {
        while self.ports.contains_key(&Port(protocol, self.next_port)) {
            self.next_port += 1;
        }
        self.next_port
    }
    fn get_port(&mut self, protocol: Protocol, port: u16) -> Result<Port> {
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

#[derive(Debug)]
pub struct Pipe<T, Data: Clone> {
    sender: Mutex<Sender<T>>,
    receiver: Mutex<Receiver<T>>,
    data: Data,
}

impl<T, Data: Clone> Pipe<T, Data> {
    fn new(data: Data) -> (Self, Self) {
        let (tx1, rx1) = unbounded();
        let (tx2, rx2) = unbounded();
        (
            Self {
                sender: Mutex::new(tx1),
                receiver: Mutex::new(rx2),
                data: data.clone(),
            },
            Self {
                sender: Mutex::new(tx2),
                receiver: Mutex::new(rx1),
                data: data,
            },
        )
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        let (first, _) = self.data.buf.as_slices();
        if first.len() > 0 {
            let to_copy = first.len().min(buf.len());
            buf[..to_copy].copy_from_slice(&first[0..to_copy]);
            self.data.buf.drain(0..to_copy);
            Ok(to_copy).into()
        } else {
            let item = {
                let mut receiver = ready!(Pin::new(&mut self.receiver.lock()).poll(cx));
                ready!(receiver.poll_next_unpin(cx))
            };
            match item {
                Some(mut data) => {
                    let to_copy = data.len().min(buf.len());
                    buf[..to_copy].copy_from_slice(&data[..to_copy]);
                    data.drain(0..to_copy);
                    self.data.buf.append(&mut data.into());
                    Ok(to_copy).into()
                }
                None => Ok(0).into(),
            }
        }
    }
}
impl AsyncWrite for TcpStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        let mut sender = ready!(Pin::new(&mut self.sender.lock()).poll(cx));
        ready!(sender.poll_ready_unpin(cx)).map_err(map_err)?;
        sender.start_send(Vec::from(buf)).map_err(map_err)?;
        Ok(buf.len()).into()
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut sender = ready!(Pin::new(&mut self.sender.lock()).poll(cx));
        ready!(sender.poll_flush_unpin(cx)).map_err(map_err)?;
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut sender = ready!(Pin::new(&mut self.sender.lock()).poll(cx));
        ready!(sender.poll_close_unpin(cx)).map_err(map_err)?;
        Poll::Ready(Ok(()))
    }
}

#[async_trait]
impl traits::TcpStream for TcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        Ok(self.data.peer_addr.into())
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.data.local_addr.into())
    }
    async fn shutdown(&self, how: Shutdown) -> Result<()> {
        match how {
            Shutdown::Read => {
                self.receiver.lock().await.close();
            }
            Shutdown::Write => {
                self.sender.lock().await.close_channel();
            }
            Shutdown::Both => {
                self.receiver.lock().await.close();
                self.sender.lock().await.close_channel();
            }
        }
        Ok(())
    }
}

#[async_trait]
impl traits::TcpListener<TcpStream> for TcpListener {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        match self.receiver.lock().await.next().await {
            Some(t) => {
                let addr = t.data.peer_addr.clone().into();
                Ok((t, addr))
            }
            None => Err(ErrorKind::ConnectionAborted.into()),
        }
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.data.into())
    }
}

#[async_trait]
impl traits::UdpSocket for UdpSocket {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        match self.receiver.lock().await.next().await {
            Some(((dat, addr))) => {
                let to_copy = buf.len().min(dat.len());
                buf[0..to_copy].clone_from_slice(&dat[0..to_copy]);
                Ok((to_copy, addr))
            }
            None => Err(ErrorKind::BrokenPipe.into()),
        }
    }
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        check_address(&addr)?;
        let mut inner = self.data.inner.lock().await;
        let target_key = Port(Protocol::Udp, addr.port());
        match inner.ports.get_mut(&target_key) {
            Some(v) => {
                let sender = v.get_udp_socket()?;
                sender
                    .sender
                    .lock()
                    .await
                    .send((Vec::from(buf), self.data.local_addr.into()))
                    .await
                    .map_err(map_err)?;
                Ok(buf.len())
            }
            None => Err(ErrorKind::ConnectionReset.into()),
        }
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.data.local_addr.into())
    }
}

#[async_trait]
impl<PR: Unpin + Send + Sync> ProxyTcpListener for VirtualHost<PR> {
    type TcpStream = TcpStream;
    type TcpListener = TcpListener;

    async fn tcp_bind<A: traits::IntoAddress>(&self, addr: A) -> Result<Self::TcpListener> {
        let addr = self.resolve(addr).await?;
        check_address(&addr)?;
        let mut inner = self.inner.lock().await;
        let key = inner.get_port(Protocol::Tcp, addr.port())?;
        let (listener, sender) = TcpListener::new(key);
        inner.ports.insert(key, Value::TcpListener(sender));
        Ok(listener)
    }
}

#[async_trait]
impl<PR: Unpin + Send + Sync> ProxyTcpStream for VirtualHost<PR> {
    type TcpStream = TcpStream;

    async fn tcp_connect<A: traits::IntoAddress>(&self, addr: A) -> Result<Self::TcpStream> {
        let addr = self.resolve(addr).await?;
        check_address(&addr)?;
        let mut inner = self.inner.lock().await;
        let target_key = Port(Protocol::Tcp, addr.port());
        let key = inner.get_port(Protocol::Tcp, 0)?;
        match inner.ports.get_mut(&target_key) {
            Some(v) => {
                let sender = v.get_tcp_listener()?;
                let (tcp_socket, mut other) = TcpStream::new(TcpData {
                    buf: VecDeque::new(),
                    local_addr: key,
                    peer_addr: target_key,
                });
                other.data.swap();
                sender
                    .sender
                    .lock()
                    .await
                    .send(other)
                    .await
                    .map_err(map_err)?;
                Ok(tcp_socket)
            }
            None => Err(ErrorKind::ConnectionRefused.into()),
        }
    }
}

#[async_trait]
impl<PR: Unpin + Send + Sync> ProxyUdpSocket for VirtualHost<PR> {
    type UdpSocket = UdpSocket;

    async fn udp_bind<A: traits::IntoAddress>(&self, addr: A) -> Result<Self::UdpSocket> {
        let addr = self.resolve(addr).await?;
        check_address(&addr)?;
        let mut inner = self.inner.lock().await;
        let key = inner.get_port(Protocol::Udp, addr.port())?;
        let (udp_socket, other) = UdpSocket::new(UdpData {
            inner: self.inner.clone(),
            local_addr: key,
        });
        inner.ports.insert(key, Value::UdpSocket(other));
        Ok(udp_socket)
    }
}

#[async_trait]
impl<PR: Unpin + Send + Sync> ProxyResolver for VirtualHost<PR> {
    async fn resolve_domain(&self, (domain, port): (&str, u16)) -> Result<SocketAddr> {
        match domain {
            "localhost" => Ok(SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), port)),
            _ => Err(ErrorKind::AddrNotAvailable.into()),
        }
    }
}

#[async_trait]
impl<PR: Runtime> Runtime for VirtualHost<PR> {
    fn spawn<Fut>(&self, future: Fut)
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send,
    {
        PR::spawn(self.pr.as_ref().as_ref().unwrap(), future)
    }
    async fn sleep(&self, duration: Duration) {
        PR::sleep(self.pr.as_ref().as_ref().unwrap(), duration).await
    }
}

fn check_address(addr: &SocketAddr) -> Result<()> {
    if addr.ip().is_loopback() {
        Ok(())
    } else if addr.ip().is_unspecified() {
        Ok(())
    } else {
        Err(ErrorKind::AddrNotAvailable.into())
    }
}

fn map_err(_e: SendError) -> Error {
    ErrorKind::BrokenPipe.into()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{TcpListener, TcpStream, UdpSocket, *};
    use crate::ActiveRT;
    use futures::prelude::*;

    #[tokio::test]
    async fn test_tcp() -> std::io::Result<()> {
        let rt = ActiveRT;
        let vh = VirtualHost::with_pr(rt);
        let vh2 = vh.clone();
        let handle = crate::tests::echo_server(vh2, ("127.0.0.1", 1234)).await?;

        let mut client = vh.tcp_connect(("127.0.0.1", 1234)).await?;
        client.write_all(b"hello").await.unwrap();
        client.close().await.unwrap();
        let mut buf = Vec::new();
        client.read_to_end(&mut buf).await.unwrap();
        println!("{:?}", buf);
        handle.await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_tcp_listener_local_addr() {
        let vh = VirtualHost::new();
        let addr = "127.0.0.1:12345".parse().unwrap();
        let socket = vh.tcp_bind(addr).await.unwrap();

        assert_eq!(socket.local_addr().await.unwrap(), addr);

        let vh = VirtualHost::new();
        let socket = vh.tcp_bind("0.0.0.0:0").await.unwrap();
        assert_eq!(
            socket.local_addr().await.unwrap(),
            "127.0.0.1:1".parse().unwrap()
        );
    }

    #[tokio::test]
    async fn test_tcp_stream_addr() {
        let vh = VirtualHost::new();
        let addr = "127.0.0.1:12345".parse().unwrap();
        let server = vh.tcp_bind(addr).await.unwrap();

        let socket = vh.tcp_connect(addr).await.unwrap();
        let (accepted, accepted_addr) = server.accept().await.unwrap();

        assert_eq!(socket.peer_addr().await.unwrap(), addr);
        assert_eq!(
            socket.local_addr().await.unwrap(),
            "127.0.0.1:1".parse().unwrap()
        );

        assert_eq!(accepted.local_addr().await.unwrap(), addr);
        assert_eq!(
            accepted.peer_addr().await.unwrap(),
            "127.0.0.1:1".parse().unwrap()
        );
        assert_eq!(accepted_addr, "127.0.0.1:1".parse().unwrap())
    }

    #[tokio::test]
    async fn test_udp() -> std::io::Result<()> {
        let addr = "127.0.0.1:1234".parse().unwrap();

        let vh = VirtualHost::with_pr(ActiveRT);
        let handle = crate::tests::echo_server_udp(vh.clone(), addr).await?;
        let socket = vh.udp_bind("0.0.0.0:0").await?;

        socket.send_to(b"hello", addr).await?;
        let mut buf = [0u8; 1024];
        let (size, addr) = socket.recv_from(&mut buf).await?;

        assert_eq!(&buf[..size], b"hello");
        assert_eq!(socket.local_addr().await?, "127.0.0.1:1".parse().unwrap());

        handle.await?;
        Ok(())
    }
}
