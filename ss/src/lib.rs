use rd_interface::{
    async_trait, Address, INet, Registry, Result, TcpListener, TcpStream, UdpSocket,
    NOT_IMPLEMENTED,
};

pub struct Net;

impl Net {
    fn new() -> Net {
        Net
    }
}

#[async_trait]
impl INet for Net {
    async fn tcp_connect(&self, _addr: Address) -> Result<TcpStream> {
        Err(NOT_IMPLEMENTED)
    }

    async fn tcp_bind(&self, _addr: Address) -> Result<TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(&self, _addr: Address) -> Result<UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
}

#[no_mangle]
pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_net("shadowsocks", |_, _| Ok(Net::new()));

    Ok(())
}
