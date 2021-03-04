use std::collections::HashMap;

use rd_interface::{
    async_trait, Address, Arc, Context, INet, Net, Result, TcpListener, TcpStream, UdpSocket,
    NOT_IMPLEMENTED,
};

pub struct Rule {
    net: HashMap<String, Net>,
}

impl Rule {
    pub fn new(net: HashMap<String, Net>) -> Net {
        Arc::new(Rule { net })
    }
}

#[async_trait]
impl INet for Rule {
    async fn tcp_connect(&self, ctx: &mut Context, addr: Address) -> Result<TcpStream> {
        Err(NOT_IMPLEMENTED)
    }

    async fn tcp_bind(&self, ctx: &mut Context, addr: Address) -> Result<TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: Address) -> Result<UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
}
