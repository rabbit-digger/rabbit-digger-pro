use rd_interface::{
    async_trait,
    registry::{NetFactory, NetRef},
    schemars::{self, JsonSchema},
    Address, Config, Context, Error, INet, Net, Registry, Result, TcpListener, TcpStream,
    UdpSocket,
};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Config, JsonSchema)]
pub struct SelectConfig {
    net_list: Vec<NetRef>,
}

pub struct SelectNet(Vec<Net>);

impl SelectNet {
    pub fn new(config: SelectConfig) -> Result<Self> {
        let nets = config
            .net_list
            .into_iter()
            .map(|v| v.net())
            .collect::<Vec<_>>();
        if nets.len() == 0 {
            return Err(Error::Other("net_list is required".into()));
        }
        Ok(SelectNet(nets))
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

impl NetFactory for SelectNet {
    const NAME: &'static str = "select";
    type Config = SelectConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        SelectNet::new(config)
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<SelectNet>();
    Ok(())
}
