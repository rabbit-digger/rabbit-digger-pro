use std::collections::HashMap;

use rd_interface::{
    async_trait, Address, Context, INet, IntoDyn, Net, Result, TcpListener, TcpStream, UdpSocket,
};

pub struct SelectNet(Vec<Net>);

impl SelectNet {
    pub fn new(net: HashMap<String, Net>, list: Vec<String>) -> anyhow::Result<Net> {
        let nets = list
            .into_iter()
            .map(|target| {
                Ok(net
                    .get(&target)
                    .ok_or(anyhow::anyhow!("target is not found: {}", target))?
                    .to_owned())
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(SelectNet(nets).into_dyn())
    }
    async fn get(&self, _ctx: &Context) -> Result<&Net> {
        let mut index: usize = 0;

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
