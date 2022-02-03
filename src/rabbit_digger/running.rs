use std::{
    fmt::Debug,
    io,
    mem::replace,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
};

use futures::{ready, FutureExt, SinkExt, Stream, StreamExt};
use rd_interface::{
    async_trait,
    context::common_field::{DestDomain, DestSocketAddr},
    Address, AddressDomain, Arc, AsyncRead, AsyncWrite, Bytes, BytesMut, Context, INet, IUdpSocket,
    IntoDyn, Net, ReadBuf, Result, Sink, TcpListener, TcpStream, UdpSocket, Value,
};
use tokio::{
    sync::{oneshot, RwLock, Semaphore},
    task::JoinHandle,
};
use tracing::instrument;

use crate::Registry;

use super::{
    connection::{Connection, ConnectionConfig},
    event::EventType,
};

pub struct RunningNet {
    name: String,
    net: RwLock<Net>,
}

impl RunningNet {
    pub fn new(name: String, net: Net) -> Arc<RunningNet> {
        Arc::new(RunningNet {
            name,
            net: RwLock::new(net),
        })
    }
    pub async fn update_net(&self, net: Net) {
        *self.net.write().await = net;
    }
    pub fn net(self: &Arc<Self>) -> Net {
        self.clone()
    }
}

impl Debug for RunningNet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningNet")
            .field("name", &self.name)
            .finish()
    }
}

#[async_trait]
impl INet for RunningNet {
    #[instrument(err)]
    async fn tcp_connect(&self, ctx: &mut Context, addr: &Address) -> Result<TcpStream> {
        ctx.append_net(&self.name);
        self.net.read().await.tcp_connect(ctx, addr).await
    }

    #[instrument(err)]
    async fn tcp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<TcpListener> {
        ctx.append_net(&self.name);
        self.net.read().await.tcp_bind(ctx, addr).await
    }

    #[instrument(err)]
    async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        ctx.append_net(&self.name);
        self.net.read().await.udp_bind(ctx, addr).await
    }

    #[instrument(err)]
    async fn lookup_host(&self, addr: &Address) -> Result<Vec<SocketAddr>> {
        self.net.read().await.lookup_host(addr).await
    }
}

pub struct RunningServerNet {
    server_name: String,
    net: Net,
    config: ConnectionConfig,
}

impl RunningServerNet {
    pub fn new(server_name: String, net: Net, config: ConnectionConfig) -> RunningServerNet {
        RunningServerNet {
            server_name,
            net,
            config,
        }
    }
}

impl Debug for RunningServerNet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningServerNet")
            .field("server_name", &self.server_name)
            .finish()
    }
}

#[async_trait]
impl INet for RunningServerNet {
    #[instrument(err, skip(ctx))]
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> rd_interface::Result<TcpStream> {
        ctx.append_net(self.server_name.clone());
        // prepare context
        match addr {
            Address::Domain(domain, port) => ctx.insert_common(DestDomain(AddressDomain {
                domain: domain.to_string(),
                port: *port,
            }))?,
            Address::SocketAddr(addr) => ctx.insert_common(DestSocketAddr(*addr))?,
        };

        let tcp = self.net.tcp_connect(ctx, &addr).await?;

        tracing::info!(target: "rabbit_digger", ?ctx, "Connected");
        let tcp = WrapTcpStream::new(tcp, self.config.clone(), addr.clone(), ctx);
        Ok(tcp.into_dyn())
    }

    // TODO: wrap TcpListener
    #[instrument(err)]
    async fn tcp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> rd_interface::Result<TcpListener> {
        ctx.append_net(self.server_name.clone());

        self.net.tcp_bind(ctx, addr).await
    }

    #[instrument(err)]
    async fn udp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> rd_interface::Result<UdpSocket> {
        ctx.append_net(self.server_name.clone());

        let udp = WrapUdpSocket::new(
            self.net.udp_bind(ctx, addr).await?,
            self.config.clone(),
            addr.clone(),
            ctx,
        );
        Ok(udp.into_dyn())
    }

    #[instrument(err)]
    async fn lookup_host(&self, addr: &Address) -> Result<Vec<SocketAddr>> {
        self.net.lookup_host(addr).await
    }
}

pub struct WrapUdpSocket {
    inner: UdpSocket,
    conn: Connection,
    stopped: parking_lot::Mutex<oneshot::Receiver<()>>,
}

impl WrapUdpSocket {
    pub fn new(
        inner: UdpSocket,
        config: ConnectionConfig,
        addr: Address,
        ctx: &Context,
    ) -> WrapUdpSocket {
        let (sender, stopped) = oneshot::channel();
        WrapUdpSocket {
            inner,
            conn: Connection::new(config, EventType::NewUdp(addr, ctx.to_value(), sender)),
            stopped: parking_lot::Mutex::new(stopped),
        }
    }
}

impl Stream for WrapUdpSocket {
    type Item = io::Result<(BytesMut, SocketAddr)>;
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<io::Result<(BytesMut, SocketAddr)>>> {
        let WrapUdpSocket {
            inner,
            conn,
            stopped,
        } = &mut *self;
        if let Poll::Ready(_) = stopped.lock().poll_unpin(cx) {
            return Poll::Ready(Some(Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Aborted by user",
            )
            .into())));
        }
        let (buf, addr) = match ready!(inner.poll_next_unpin(cx)?) {
            Some(v) => v,
            None => return Poll::Ready(None),
        };
        conn.send(EventType::UdpInbound(addr.into(), buf.len() as u64));
        Poll::Ready(Some(Ok((buf, addr))))
    }
}

impl Sink<(Bytes, Address)> for WrapUdpSocket {
    type Error = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready_unpin(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: (Bytes, Address)) -> Result<(), Self::Error> {
        self.conn
            .send(EventType::UdpOutbound(item.1.clone(), item.0.len() as u64));
        self.inner.start_send_unpin(item)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_flush_unpin(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_close_unpin(cx)
    }
}

#[async_trait]
impl IUdpSocket for WrapUdpSocket {
    async fn local_addr(&self) -> Result<SocketAddr> {
        self.inner.local_addr().await
    }
}

pub struct WrapTcpStream {
    inner: TcpStream,
    conn: Connection,
    stopped: oneshot::Receiver<()>,
}

impl WrapTcpStream {
    pub fn new(
        inner: TcpStream,
        config: ConnectionConfig,
        addr: Address,
        ctx: &Context,
    ) -> WrapTcpStream {
        let (sender, stopped) = oneshot::channel();
        WrapTcpStream {
            inner,
            conn: Connection::new(config, EventType::NewTcp(addr, ctx.to_value(), sender)),
            stopped,
        }
    }
}

impl AsyncRead for WrapTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        if let Poll::Ready(_) = self.stopped.poll_unpin(cx) {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Aborted by user",
            )));
        }
        let before = buf.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let s = buf.filled().len() - before;
                self.conn.send(EventType::Inbound(s as u64));
                Ok(()).into()
            }
            r => r,
        }
    }
}

impl AsyncWrite for WrapTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match Pin::new(&mut self.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(s)) => {
                self.conn.send(EventType::Outbound(s as u64));
                Ok(s).into()
            }
            r => r,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

#[async_trait]
impl rd_interface::ITcpStream for WrapTcpStream {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        self.inner.peer_addr().await
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.inner.local_addr().await
    }
}

enum State {
    WaitConfig,
    Running {
        opt: Value,
        handle: JoinHandle<anyhow::Result<()>>,
        semaphore: Arc<Semaphore>,
    },
    Finished {
        result: anyhow::Result<()>,
    },
}

#[derive(Clone)]
pub struct RunningServer {
    #[allow(dead_code)]
    name: String,
    server_type: String,
    net: Net,
    listen: Net,
    state: Arc<RwLock<State>>,
}

impl RunningServer {
    pub fn new(name: String, server_type: String, net: Net, listen: Net) -> Self {
        RunningServer {
            name,
            server_type,
            net,
            listen,
            state: Arc::new(RwLock::new(State::WaitConfig)),
        }
    }
    pub async fn start(&self, registry: &Registry, opt: &Value) -> anyhow::Result<()> {
        match &*self.state.read().await {
            // skip if config is not changed
            State::Running {
                opt: running_opt, ..
            } => {
                if opt == running_opt {
                    return Ok(());
                }
            }
            _ => {}
        };

        self.stop().await?;

        let item = registry.get_server(&self.server_type)?;
        let server = item.build(self.listen.clone(), self.net.clone(), opt.clone())?;
        let semaphore = Arc::new(Semaphore::new(0));
        let s2 = semaphore.clone();
        let handle = tokio::spawn(async move {
            let r = server.start().await.map_err(Into::into);
            s2.close();
            r
        });

        *self.state.write().await = State::Running {
            opt: opt.clone(),
            handle,
            semaphore,
        };

        Ok(())
    }
    pub async fn stop(&self) -> anyhow::Result<()> {
        match &*self.state.read().await {
            State::Running {
                handle, semaphore, ..
            } => {
                handle.abort();
                semaphore.close();
            }
            _ => return Ok(()),
        };
        self.join().await;

        Ok(())
    }
    pub async fn join(&self) {
        let state = self.state.read().await;

        match &*state {
            State::Running { semaphore, .. } => {
                let _ = semaphore.acquire().await;
            }
            _ => return,
        };
        drop(state);

        let mut state = self.state.write().await;

        let result = match &mut *state {
            State::Running { handle, .. } => handle.await.map_err(Into::into).and_then(|i| i),
            _ => return,
        };

        *state = State::Finished { result };
    }
    pub async fn take_result(&self) -> Option<anyhow::Result<()>> {
        let mut state = self.state.write().await;

        match &*state {
            State::Finished { .. } => {
                let old = replace(&mut *state, State::WaitConfig);
                return match old {
                    State::Finished { result, .. } => Some(result),
                    _ => unreachable!(),
                };
            }
            _ => return None,
        };
    }
}
