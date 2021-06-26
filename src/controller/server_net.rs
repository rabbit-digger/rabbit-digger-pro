use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use super::{
    connection::{Connection, ConnectionConfig},
    event::EventType,
};
use rd_interface::{
    async_trait, Address, AsyncRead, AsyncWrite, INet, IntoDyn, Net, ReadBuf, TcpListener,
    UdpSocket,
};

pub struct ControllerServerNet {
    pub net: Net,
    pub config: ConnectionConfig,
}

#[async_trait]
impl INet for ControllerServerNet {
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> rd_interface::Result<rd_interface::TcpStream> {
        let src = ctx
            .get_source_addr()
            .map(|s| s.to_string())
            .unwrap_or_default();

        tracing::info!("{:?} {} -> {}", &ctx.net_list(), &src, &addr,);

        let tcp = self.net.tcp_connect(ctx, &addr).await?;
        let tcp = TcpStream::new(tcp, self.config.clone(), addr.clone());
        Ok(tcp.into_dyn())
    }

    // TODO: wrap TcpListener
    async fn tcp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> rd_interface::Result<TcpListener> {
        self.net.tcp_bind(ctx, addr).await
    }

    // TODO: wrap UdpSocket
    async fn udp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> rd_interface::Result<UdpSocket> {
        self.net.udp_bind(ctx, addr).await
    }
}

pub struct TcpStream {
    inner: rd_interface::TcpStream,
    conn: Connection,
}

impl TcpStream {
    pub fn new(
        inner: rd_interface::TcpStream,
        config: ConnectionConfig,
        addr: Address,
    ) -> TcpStream {
        TcpStream {
            inner,
            conn: Connection::new(config, EventType::NewTcp(addr)),
        }
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let s = buf.filled().len() - before;
                self.conn.send(EventType::Inbound(s));
                Ok(()).into()
            }
            r => r,
        }
    }
}
impl AsyncWrite for TcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match Pin::new(&mut self.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(s)) => {
                self.conn.send(EventType::Outbound(s));
                Ok(s).into()
            }
            r => r,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[async_trait]
impl rd_interface::ITcpStream for TcpStream {
    async fn peer_addr(&self) -> rd_interface::Result<SocketAddr> {
        self.inner.peer_addr().await
    }

    async fn local_addr(&self) -> rd_interface::Result<SocketAddr> {
        self.inner.local_addr().await
    }
}
