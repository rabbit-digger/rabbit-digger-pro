use futures::future::BoxFuture;
use rd_interface::{
    async_trait, util::get_one_net, Address, Context, INet, Registry, Result, TcpListener,
    TcpStream, UdpSocket,
};

pub struct Net(rd_interface::Net);

impl Net {
    fn new(net: rd_interface::Net) -> Net {
        Net(net)
    }
}

#[async_trait]
impl INet for Net {
    #[inline(always)]
    fn tcp_connect<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<TcpStream>>
    where
        Self: 'a,
    {
        self.0.tcp_connect(ctx, addr)
    }

    #[inline(always)]
    fn tcp_bind<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<TcpListener>>
    where
        Self: 'a,
    {
        self.0.tcp_bind(ctx, addr)
    }

    #[inline(always)]
    fn udp_bind<'life0: 'a, 'life1: 'a, 'a>(
        &'life0 self,
        ctx: &'life1 mut Context,
        addr: Address,
    ) -> BoxFuture<'a, Result<UdpSocket>>
    where
        Self: 'a,
    {
        self.0.udp_bind(ctx, addr)
    }
}

pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_net("alias", |nets, _| Ok(Net::new(get_one_net(nets)?)));
    Ok(())
}
