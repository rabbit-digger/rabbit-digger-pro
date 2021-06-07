use rd_interface::{async_trait, Address, INet, Net, TcpListener, UdpSocket};

pub struct ControllerNet {
    pub net_name: String,
    pub net: Net,
}

#[async_trait]
impl INet for ControllerNet {
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: Address,
    ) -> rd_interface::Result<rd_interface::TcpStream> {
        ctx.append_net(&self.net_name);
        self.net.tcp_connect(ctx, addr.clone()).await
    }

    // TODO: wrap TcpListener
    async fn tcp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: Address,
    ) -> rd_interface::Result<TcpListener> {
        ctx.append_net(&self.net_name);
        self.net.tcp_bind(ctx, addr).await
    }

    // TODO: wrap UdpSocket
    async fn udp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        addr: Address,
    ) -> rd_interface::Result<UdpSocket> {
        ctx.append_net(&self.net_name);
        self.net.udp_bind(ctx, addr).await
    }
}
