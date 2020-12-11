//! Virtual host (WIP)

#![allow(dead_code, unused_variables)]

use crate::traits::{self, async_trait, ProxyTcpListener, ProxyTcpStream};
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
};
use traits::ProxyUdpSocket;

pub type TcpStream = Pipe<Vec<u8>, (VecDeque<u8>, Port)>;
pub type TcpListener = Pipe<TcpStream, Port>;
pub type UdpSocket = Pipe<(Vec<u8>, SocketAddr), Port>;

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

impl Into<SocketAddr> for Port {
    fn into(self) -> SocketAddr {
        SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), self.1)
    }
}

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
        let (first, _) = self.data.0.as_slices();
        if first.len() > 0 {
            let to_copy = first.len().min(buf.len());
            buf[..to_copy].copy_from_slice(&first[0..to_copy]);
            self.data.0.drain(0..to_copy);
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
                    self.data.0.append(&mut data.into());
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
        match self.receiver.lock().await.next().await {
            Some(t) => {
                let addr = t.data.clone().1.into();
                Ok((t, addr))
            }
            None => Err(ErrorKind::ConnectionAborted.into()),
        }
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
        let key = inner.get_port(Protocol::Tcp, addr.port())?;
        let (listener, sender) = TcpListener::new(key);
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
        let target_key = Port(Protocol::Tcp, addr.port());
        let key = inner.get_port(Protocol::Tcp, 0)?;
        match inner.ports.get_mut(&target_key) {
            Some(v) => {
                let sender = v.get_tcp_listener()?;
                let (tcp_socket, other) = TcpStream::new((VecDeque::new(), key));
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
impl ProxyUdpSocket for VirtualHost {
    type UdpSocket = UdpSocket;

    async fn udp_bind(&self, addr: SocketAddr) -> Result<Self::UdpSocket> {
        check_address(&addr)?;
        let mut inner = self.inner.lock().await;
        let key = inner.get_port(Protocol::Udp, addr.port())?;
        let (udp_socket, other) = UdpSocket::new(key);
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
    use crate::prelude::{TcpListener, *};
    use crate::Tokio;
    use futures::prelude::*;

    #[tokio::test]
    async fn test_tcp() -> std::io::Result<()> {
        let tk = Tokio;
        let pr = VirtualHost::new();
        let server = pr.tcp_bind("127.0.0.1:1234".parse().unwrap()).await?;
        let handle = tk.spawn_handle(async move {
            let (mut socket, addr) = server.accept().await.unwrap();
            println!("accept from {}", addr);
            let mut buf = Vec::new();
            socket.read_to_end(&mut buf).await.unwrap();
            println!("server recv {:?}", buf);
            socket.write_all(&buf).await.unwrap();
        });

        let addr = "127.0.0.1:1234".parse().unwrap();
        let mut client = pr.tcp_connect(addr).await?;
        client.write_all(b"hello").await.unwrap();
        client.close().await.unwrap();
        let mut buf = Vec::new();
        client.read_to_end(&mut buf).await.unwrap();
        println!("{:?}", buf);
        handle.await;
        Ok(())
    }
}
