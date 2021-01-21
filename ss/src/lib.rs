use rd_interface::{
    async_trait, Address, INet, Registry, Result, TcpListener, TcpStream, UdpSocket,
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
        todo!()
    }

    async fn tcp_bind(&self, _addr: Address) -> Result<TcpListener> {
        todo!()
    }

    async fn udp_bind(&self, _addr: Address) -> Result<UdpSocket> {
        todo!()
    }
}

#[no_mangle]
pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_net("shadowsocks", |_, _| Ok(Net::new()));

    Ok(())
}
