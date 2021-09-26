use rd_interface::{
    async_trait,
    prelude::*,
    registry::{NetFactory, NetRef},
    Address, Context, Error, INet, Net, Registry, Result, TcpListener, TcpStream, UdpSocket,
};

#[rd_config]
#[derive(Debug, Clone)]
pub struct SelectNetConfig {
    selected: NetRef,
    list: Vec<NetRef>,
}

pub struct SelectNet(Option<Net>);

impl SelectNet {
    pub fn new(config: SelectNetConfig) -> Result<Self> {
        if config.list.is_empty() {
            return Err(Error::Other("select list is empty".into()));
        }

        Ok(SelectNet(Some((*config.selected).clone())))
    }
    fn net(&self) -> Result<&Net> {
        self.0
            .as_ref()
            .ok_or_else(|| Error::Other(format!("select net not found").into()))
    }
}

#[async_trait]
impl INet for SelectNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: &Address) -> Result<TcpStream> {
        self.net()?.tcp_connect(ctx, addr).await
    }

    async fn tcp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<TcpListener> {
        self.net()?.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        self.net()?.udp_bind(ctx, addr).await
    }

    async fn lookup_host(&self, addr: &Address) -> Result<Vec<std::net::SocketAddr>> {
        self.net()?.lookup_host(addr).await
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
