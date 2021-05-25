use rd_interface::{
    async_trait,
    registry::{NetFactory, NetRef},
    schemars::{self, JsonSchema},
    Address, Config, Context, INet, Net, Registry, Result, TcpListener, TcpStream, UdpSocket,
};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Config, JsonSchema)]
pub struct SelectConfig {
    selected: NetRef,
    list: Vec<String>,
}

pub struct SelectNet(Net);

impl SelectNet {
    pub fn new(config: SelectConfig) -> Result<Self> {
        Ok(SelectNet(config.selected.net()))
    }
}

#[async_trait]
impl INet for SelectNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: Address) -> Result<TcpStream> {
        self.0.tcp_connect(ctx, addr).await
    }

    async fn tcp_bind(&self, ctx: &mut Context, addr: Address) -> Result<TcpListener> {
        self.0.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: Address) -> Result<UdpSocket> {
        self.0.udp_bind(ctx, addr).await
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
