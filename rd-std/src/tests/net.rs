use rd_interface::{
    async_trait, Address, Arc, Bytes, Context, INet, Result, TcpListener, TcpStream, UdpSocket,
};
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::mpsc::{channel, Receiver, Sender};

struct Inner {
    tcp: HashMap<u16, Sender<Bytes>>,
    udp: HashMap<u16, Sender<(Bytes, SocketAddr)>>,
}

/// A test net that can be used for testing.
/// It simulates a network on a localhost, without any real network.
pub struct TestNet {
    inner: Arc<Inner>,
}

#[async_trait]
impl INet for TestNet {
    async fn tcp_connect(&self, _ctx: &mut Context, _addr: &Address) -> Result<TcpStream> {
        Err(rd_interface::NOT_IMPLEMENTED)
    }

    async fn tcp_bind(&self, _ctx: &mut Context, _addr: &Address) -> Result<TcpListener> {
        Err(rd_interface::NOT_IMPLEMENTED)
    }

    async fn udp_bind(&self, _ctx: &mut Context, _addr: &Address) -> Result<UdpSocket> {
        Err(rd_interface::NOT_IMPLEMENTED)
    }

    async fn lookup_host(&self, _addr: &Address) -> Result<Vec<SocketAddr>> {
        Err(rd_interface::NOT_IMPLEMENTED)
    }
}
