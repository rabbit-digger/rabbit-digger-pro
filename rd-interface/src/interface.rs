#[cfg(unix)]
use std::os::unix::prelude::RawFd;
use std::{
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
};

pub use crate::Context;
use crate::NOT_IMPLEMENTED;
pub use crate::{Address, Error, Result};
pub use async_trait::async_trait;
use futures_util::future::poll_fn;
pub use std::sync::Arc;
use std::{any::Any, io};
pub use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub trait IntoDyn<DynType> {
    fn into_dyn(self) -> DynType
    where
        Self: Sized + 'static;
}

/// A TcpListener.
#[async_trait]
pub trait ITcpListener: Unpin + Send + Sync {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub type TcpListener = Box<dyn ITcpListener>;

impl<T: ITcpListener> IntoDyn<TcpListener> for T {
    fn into_dyn(self) -> TcpListener
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Fd {
    #[cfg(unix)]
    Unix(RawFd),
}

#[cfg(unix)]
impl From<RawFd> for Fd {
    fn from(fd: RawFd) -> Self {
        Fd::Unix(fd)
    }
}

/// A TcpStream.
#[async_trait]
pub trait ITcpStream: Unpin + Send + Sync {
    fn poll_read(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>>;
    fn poll_write(&mut self, cx: &mut task::Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>>;
    fn poll_flush(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<()>>;
    fn poll_shutdown(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<()>>;
    fn read_passthrough(&self) -> Option<Fd> {
        None
    }
    fn write_passthrough(&self) -> Option<Fd> {
        None
    }

    async fn peer_addr(&self) -> Result<SocketAddr>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub struct TcpStream(Box<dyn ITcpStream>);

impl TcpStream {
    pub async fn peer_addr(&self) -> Result<SocketAddr> {
        self.0.peer_addr().await
    }
    pub async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().await
    }
    pub fn read_passthrough(&self) -> Option<Fd> {
        self.0.read_passthrough()
    }
    pub fn write_passthrough(&self) -> Option<Fd> {
        self.0.write_passthrough()
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.0.poll_read(cx, buf)
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.0.poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        self.0.poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        self.0.poll_shutdown(cx)
    }
}

impl<T: ITcpStream> IntoDyn<TcpStream> for T {
    fn into_dyn(self) -> TcpStream
    where
        Self: Sized + 'static,
    {
        TcpStream(Box::new(self))
    }
}

/// A UdpSocket.
#[async_trait]
pub trait IUdpSocket: Unpin + Send + Sync {
    fn poll_recv_from(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<SocketAddr>>;
    fn poll_send_to(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &[u8],
        target: &Address,
    ) -> Poll<io::Result<usize>>;
    async fn local_addr(&self) -> Result<SocketAddr>;
}
pub struct UdpSocket(Box<dyn IUdpSocket>);

impl<T: IUdpSocket> IntoDyn<UdpSocket> for T {
    fn into_dyn(self) -> UdpSocket
    where
        Self: Sized + 'static,
    {
        UdpSocket(Box::new(self))
    }
}

impl UdpSocket {
    pub async fn recv_from(&mut self, buf: &mut ReadBuf<'_>) -> Result<SocketAddr> {
        poll_fn(|cx| self.0.poll_recv_from(cx, buf))
            .await
            .map_err(Into::into)
    }
    pub async fn send_to(&mut self, buf: &[u8], target: &Address) -> Result<usize> {
        poll_fn(|cx| self.0.poll_send_to(cx, buf, target))
            .await
            .map_err(Into::into)
    }
    pub async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().await
    }
    pub fn poll_recv_from(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<SocketAddr>> {
        self.0.poll_recv_from(cx, buf)
    }
    pub fn poll_send_to(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &[u8],
        target: &Address,
    ) -> Poll<io::Result<usize>> {
        self.0.poll_send_to(cx, buf, target)
    }
}

// It's from crate downcast-rs
pub trait Downcast: Send + Sync {
    /// Convert `Arc<Trait>` (where `Trait: Downcast`) to `Arc<Any>`. `Arc<Any>` can then be
    /// further `downcast` into `Arc<ConcreteType>` where `ConcreteType` implements `Trait`.
    fn into_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
}

impl<T: Any + Send + Sync> Downcast for T {
    fn into_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

/// A Net.
#[async_trait]
pub trait INet: Downcast + Unpin + Send + Sync {
    async fn tcp_connect(&self, _ctx: &mut Context, _addr: &Address) -> Result<TcpStream> {
        Err(NOT_IMPLEMENTED)
    }
    async fn tcp_bind(&self, _ctx: &mut Context, _addr: &Address) -> Result<TcpListener> {
        Err(NOT_IMPLEMENTED)
    }
    async fn udp_bind(&self, _ctx: &mut Context, _addr: &Address) -> Result<UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
    async fn lookup_host(&self, _addr: &Address) -> Result<Vec<SocketAddr>> {
        Err(NOT_IMPLEMENTED)
    }
    // It's used to downcast. Don't implement it.
    fn get_inner(&self) -> Option<Net> {
        None
    }
}

#[derive(Clone)]
pub struct Net(Arc<dyn INet>);

impl<T: INet> IntoDyn<Net> for T {
    fn into_dyn(self) -> Net
    where
        Self: Sized + 'static,
    {
        Net(Arc::new(self))
    }
}

pub trait NetExt {
    fn get_net_by_type<T: INet + 'static>(net: Net) -> Option<Arc<T>>;
}

impl From<Arc<dyn INet>> for Net {
    fn from(net: Arc<dyn INet>) -> Self {
        Net(net)
    }
}

impl Net {
    pub async fn tcp_connect(&self, ctx: &mut Context, addr: &Address) -> Result<TcpStream> {
        self.0.tcp_connect(ctx, addr).await
    }
    pub async fn tcp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<TcpListener> {
        self.0.tcp_bind(ctx, addr).await
    }
    pub async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        self.0.udp_bind(ctx, addr).await
    }
    pub async fn lookup_host(&self, addr: &Address) -> Result<Vec<SocketAddr>> {
        self.0.lookup_host(addr).await
    }
    pub fn get_inner_net_by<T: INet + 'static>(self) -> Option<Arc<T>> {
        let mut net = self.0;
        loop {
            net = match net.clone().into_any_arc().downcast() {
                Ok(t) => return Some(t),
                Err(_) => match net.get_inner() {
                    Some(n) => n.0,
                    None => return None,
                },
            }
        }
    }
    pub fn as_ptr(&self) -> *const dyn INet {
        Arc::as_ptr(&self.0)
    }
}

/// A Server.
#[async_trait]
pub trait IServer: Unpin + Send + Sync {
    /// Start the server, drop to stop.
    async fn start(&self) -> Result<()>;
}
pub struct Server(Box<dyn IServer>);

impl Server {
    pub async fn start(&self) -> Result<()> {
        self.0.start().await
    }
}

impl<T: IServer> IntoDyn<Server> for T {
    fn into_dyn(self) -> Server
    where
        Self: Sized + 'static,
    {
        Server(Box::new(self))
    }
}

/// The other side of an UdpSocket
pub trait IUdpChannel: Unpin + Send + Sync {
    fn poll_send_to(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<Address>>;
    fn poll_recv_from(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &[u8],
        target: &SocketAddr,
    ) -> Poll<io::Result<usize>>;
}
pub struct UdpChannel(Box<dyn IUdpChannel>);

impl<T: IUdpChannel> crate::IntoDyn<UdpChannel> for T {
    fn into_dyn(self) -> UdpChannel
    where
        Self: Sized + 'static,
    {
        UdpChannel(Box::new(self))
    }
}

impl UdpChannel {
    pub fn poll_send_to(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<Address>> {
        self.0.poll_send_to(cx, buf)
    }
    pub fn poll_recv_from(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &[u8],
        target: &SocketAddr,
    ) -> Poll<io::Result<usize>> {
        self.0.poll_recv_from(cx, buf, target)
    }
}
