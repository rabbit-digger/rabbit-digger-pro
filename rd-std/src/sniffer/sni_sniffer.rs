use rd_interface::{async_trait, Address, Context, INet, Net, Result};

pub struct SNISnifferNet {
    net: Net,
}

impl SNISnifferNet {
    pub fn new(net: Net) -> Self {
        Self { net }
    }
}

impl INet for SNISnifferNet {
    fn provide_tcp_connect(&self) -> Option<&dyn rd_interface::TcpConnect> {
        self.net.provide_tcp_connect()
    }

    fn provide_tcp_bind(&self) -> Option<&dyn rd_interface::TcpBind> {
        self.net.provide_tcp_bind()
    }

    fn provide_udp_bind(&self) -> Option<&dyn rd_interface::UdpBind> {
        self.net.provide_udp_bind()
    }

    fn provide_lookup_host(&self) -> Option<&dyn rd_interface::LookupHost> {
        self.net.provide_lookup_host()
    }
}

#[async_trait]
impl rd_interface::TcpConnect for SNISnifferNet {
    async fn tcp_connect(
        &self,
        ctx: &mut Context,
        addr: &Address,
    ) -> Result<rd_interface::TcpStream> {
        self.net.tcp_connect(ctx, addr).await
    }
}
