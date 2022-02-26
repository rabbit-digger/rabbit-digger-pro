use rd_interface::{
    async_trait, Address, Context, INet, Net, Result, TcpListener, TcpStream, UdpSocket,
};
use std::net::SocketAddr;

/// A no-op Net returns [`Error::NotImplemented`](crate::Error::NotImplemented) for every method.
pub struct NotImplementedNet;

#[async_trait]
impl INet for NotImplementedNet {}

/// A new Net calls [`tcp_connect()`](crate::INet::tcp_connect()), [`tcp_bind()`](crate::INet::tcp_bind()), [`udp_bind()`](crate::INet::udp_bind()) from different Net.
pub struct CombineNet {
    pub tcp_connect: Net,
    pub tcp_bind: Net,
    pub udp_bind: Net,
    pub lookup_host: Net,
}

#[async_trait]
impl INet for CombineNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: &Address) -> Result<TcpStream> {
        self.tcp_connect.tcp_connect(ctx, addr).await
    }

    async fn tcp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<TcpListener> {
        self.tcp_bind.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        self.udp_bind.udp_bind(ctx, addr).await
    }

    async fn lookup_host(&self, addr: &Address) -> Result<Vec<SocketAddr>> {
        self.lookup_host.lookup_host(addr).await
    }
}

#[cfg(test)]
mod tests {
    use rd_interface::IntoDyn;
    use tokio::task::yield_now;

    use crate::tests::{
        assert_echo, assert_echo_udp, spawn_echo_server, spawn_echo_server_udp, TestNet,
    };

    use super::*;

    #[tokio::test]
    async fn test_combine_net() {
        let tcp_connect = TestNet::new().into_dyn();
        let tcp_bind = TestNet::new().into_dyn();
        let udp_bind = TestNet::new().into_dyn();
        let lookup_host = TestNet::new().into_dyn();
        let tcp_bind2 = tcp_bind.clone();
        spawn_echo_server(&tcp_connect, "127.0.0.1:12345").await;
        spawn_echo_server_udp(&udp_bind, "127.0.0.1:12346").await;

        yield_now().await;

        let net = CombineNet {
            tcp_connect,
            tcp_bind,
            udp_bind,
            lookup_host,
        }
        .into_dyn();

        assert_echo(&net, "127.0.0.1:12345").await;
        assert_echo_udp(&net, "127.0.0.1:12346").await;

        spawn_echo_server(&net, "127.0.0.1:12346").await;
        yield_now().await;
        assert_echo(&tcp_bind2, "127.0.0.1:12346").await;
    }
}
