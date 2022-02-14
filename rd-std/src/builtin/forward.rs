use std::{
    future::pending,
    io,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
};

use crate::util::{connect_tcp, connect_udp};
use futures::{ready, Sink, SinkExt, Stream, StreamExt};
use rd_interface::{
    async_trait, config::NetRef, prelude::*, registry::ServerBuilder, Address, Bytes, BytesMut,
    Context, IServer, IUdpChannel, IntoDyn, Net, Result, TcpListener, TcpStream, UdpSocket,
};
use tokio::select;
use tracing::instrument;

/// A server that forwards all connections to target.
#[rd_config]
#[derive(Debug)]
pub struct ForwardServerConfig {
    bind: Address,
    target: Address,
    #[serde(default)]
    udp: bool,
    #[serde(default)]
    net: NetRef,
    #[serde(default)]
    listen: NetRef,
}

pub struct ForwardServer {
    listen_net: Net,
    net: Net,
    bind: Address,
    target: Address,
    udp: bool,
}

impl ForwardServer {
    fn new(cfg: ForwardServerConfig) -> ForwardServer {
        ForwardServer {
            listen_net: (*cfg.listen).clone(),
            net: (*cfg.net).clone(),
            bind: cfg.bind,
            target: cfg.target,
            udp: cfg.udp,
        }
    }
}
#[async_trait]
impl IServer for ForwardServer {
    async fn start(&self) -> Result<()> {
        let listener = self
            .listen_net
            .tcp_bind(&mut Context::new(), &self.bind)
            .await?;

        let tcp_task = self.serve_listener(listener);
        let udp_task = self.serve_udp();

        select! {
            r = tcp_task => r?,
            r = udp_task => r?,
        }

        Ok(())
    }
}

impl ForwardServer {
    #[instrument(err, skip(net, socket))]
    async fn serve_connection(
        target: Address,
        socket: TcpStream,
        net: Net,
        addr: SocketAddr,
    ) -> Result<()> {
        let ctx = &mut Context::from_socketaddr(addr);
        let target = net.tcp_connect(ctx, &target).await?;
        connect_tcp(ctx, socket, target).await?;
        Ok(())
    }
    pub async fn serve_listener(&self, listener: TcpListener) -> Result<()> {
        loop {
            let (socket, addr) = listener.accept().await?;
            let net = self.net.clone();
            let target = self.target.clone();
            let _ = tokio::spawn(async move {
                if let Err(e) = Self::serve_connection(target, socket, net, addr).await {
                    tracing::error!("Error when serve_connection: {:?}", e);
                }
            });
        }
    }
    async fn serve_udp(&self) -> Result<()> {
        if !self.udp {
            pending::<()>().await;
        }

        let udp_listener = ListenUdpChannel {
            udp: self
                .listen_net
                .udp_bind(&mut Context::new(), &self.bind)
                .await?,
            client: None,
            target: self.target.clone(),
        }
        .into_dyn();

        let udp = self
            .net
            .udp_bind(&mut Context::new(), &self.target.to_any_addr_port()?)
            .await?;

        connect_udp(&mut Context::new(), udp_listener, udp).await?;

        Ok(())
    }
}

impl ServerBuilder for ForwardServer {
    const NAME: &'static str = "forward";
    type Config = ForwardServerConfig;
    type Server = Self;

    fn build(cfg: Self::Config) -> Result<Self> {
        Ok(ForwardServer::new(cfg))
    }
}

struct ListenUdpChannel {
    udp: UdpSocket,
    client: Option<SocketAddr>,
    target: Address,
}

impl Stream for ListenUdpChannel {
    type Item = io::Result<(Bytes, Address)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let item = ready!(self.udp.poll_next_unpin(cx));
        Poll::Ready(item.map(|r| {
            r.map(|(bytes, addr)| {
                self.client = Some(addr);
                return (bytes.freeze(), self.target.clone());
            })
        }))
    }
}

impl Sink<(BytesMut, SocketAddr)> for ListenUdpChannel {
    type Error = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.udp.poll_ready_unpin(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        (bytes, _): (BytesMut, SocketAddr),
    ) -> Result<(), Self::Error> {
        if let Some(client) = self.client {
            self.udp.start_send_unpin((bytes.freeze(), client.into()))
        } else {
            Ok(())
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.udp.poll_flush_unpin(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.udp.poll_close_unpin(cx)
    }
}

impl IUdpChannel for ListenUdpChannel {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rd_interface::{IntoAddress, IntoDyn};
    use tokio::time::sleep;

    use super::*;
    use crate::tests::{
        assert_echo, assert_echo_udp, spawn_echo_server, spawn_echo_server_udp, TestNet,
    };

    #[tokio::test]
    async fn test_forward_server() {
        let net = TestNet::new().into_dyn();

        let server = ForwardServer {
            listen_net: net.clone(),
            net: net.clone(),
            bind: "127.0.0.1:1234".into_address().unwrap(),
            target: "127.0.0.1:4321".into_address().unwrap(),
            udp: true,
        };
        tokio::spawn(async move { server.start().await.unwrap() });
        spawn_echo_server(&net, "127.0.0.1:4321").await;
        spawn_echo_server_udp(&net, "127.0.0.1:4321").await;

        sleep(Duration::from_millis(1)).await;

        assert_echo(&net, "127.0.0.1:1234").await;
        assert_echo_udp(&net, "127.0.0.1:1234").await;
    }
}
