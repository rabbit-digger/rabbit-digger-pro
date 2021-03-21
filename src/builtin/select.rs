use rd_interface::{
    async_trait, config::from_value, context::common_field::PersistData, Address, Context, INet,
    Net, Registry, Result, TcpListener, TcpStream, UdpSocket,
};

pub struct SelectNet(Vec<Net>);

impl SelectNet {
    fn new(nets: Vec<Net>) -> SelectNet {
        SelectNet(nets)
    }
    async fn get(&self, ctx: &Context) -> Result<&Net> {
        let data = ctx.get_common::<PersistData>().await?;
        let mut index: usize = from_value(data.0)?;

        if index > self.0.len() {
            index = 0;
        }

        Ok(&self.0[index])
    }
}

#[async_trait]
impl INet for SelectNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: Address) -> Result<TcpStream> {
        self.get(ctx).await?.tcp_connect(ctx, addr).await
    }

    async fn tcp_bind(&self, ctx: &mut Context, addr: Address) -> Result<TcpListener> {
        self.get(ctx).await?.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: Address) -> Result<UdpSocket> {
        self.get(ctx).await?.udp_bind(ctx, addr).await
    }
}

pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_net("select", |nets, _| {
        if nets.len() > 0 {
            Ok(SelectNet::new(nets))
        } else {
            Err(rd_interface::Error::Other(
                "Provide at least one net".into(),
            ))
        }
    });
    Ok(())
}
