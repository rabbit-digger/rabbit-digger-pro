use futures::future::BoxFuture;
use rd_interface::{Address, Context, INet, Registry, Result, TcpListener, TcpStream, UdpSocket};

pub struct CombineNet(rd_interface::Net, rd_interface::Net, rd_interface::Net);

impl CombineNet {
    fn new(mut nets: Vec<rd_interface::Net>) -> rd_interface::Result<CombineNet> {
        if nets.len() != 1 {
            return Err(rd_interface::Error::Other(
                "Must have one net".to_string().into(),
            ));
        }
        let net0 = nets.remove(0);
        let net1 = nets.remove(0);
        let net2 = nets.remove(0);
        Ok(CombineNet(net0, net1, net2))
    }
}

impl INet for CombineNet {
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
        self.1.tcp_bind(ctx, addr)
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
        self.2.udp_bind(ctx, addr)
    }
}

pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_net("combine", |nets, _| CombineNet::new(nets));
    Ok(())
}
