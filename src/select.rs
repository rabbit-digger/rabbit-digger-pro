use rd_interface::{
    async_trait,
    registry::{NetFactory, NetRef},
    schemars::{self, JsonSchema},
    Address, Config, Context, Error, INet, Net, Registry, Result, TcpListener, TcpStream,
    UdpSocket,
};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Config, JsonSchema)]
pub struct SelectNetConfig {
    selected: usize,
    list: Vec<NetRef>,
}

pub struct SelectNet(Net);

impl SelectNet {
    pub fn new(config: SelectNetConfig) -> Result<Self> {
        if config.list.is_empty() {
            return Err(Error::Other("select list is empty".into()));
        }
        let index = config.selected.min(config.list.len() - 1);
        let net = &config.list[index];
        tracing::trace!("selected net: {}", net.name());
        let net = net.net();
        Ok(SelectNet(net))
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
    type Config = SelectNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        SelectNet::new(config)
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<SelectNet>();
    Ok(())
}
