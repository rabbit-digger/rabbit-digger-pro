use std::{
    io::{self, ErrorKind},
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use rd_interface::{
    async_trait, Address, BoxTcpListener, BoxTcpStream, BoxUdpSocket, Plugin, ProxyNet, Registry,
    Result,
};
pub struct Net;

impl Net {
    fn new() -> Net {
        Net
    }
}

#[async_trait]
impl ProxyNet for Net {
    async fn tcp_connect(&self, _addr: Address) -> Result<BoxTcpStream> {
        todo!()
    }

    async fn tcp_bind(&self, _addr: Address) -> Result<BoxTcpListener> {
        todo!()
    }

    async fn udp_bind(&self, _addr: Address) -> Result<BoxUdpSocket> {
        todo!()
    }
}

#[no_mangle]
pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_plugin("socks5", Plugin::Net(Box::new(Net::new())));
    Ok(())
}
