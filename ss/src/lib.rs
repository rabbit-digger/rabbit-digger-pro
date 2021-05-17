mod udp;
mod wrapper;

use rd_interface::{
    async_trait,
    registry::{NetFactory, NetRef},
    schemars::{self, JsonSchema},
    Address, Arc, Config, INet, IntoAddress, IntoDyn, Registry, Result, TcpListener, TcpStream,
    UdpSocket, NOT_ENABLED, NOT_IMPLEMENTED,
};
use serde_derive::Deserialize;
use shadowsocks::{
    config::{ServerConfig, ServerType},
    context::Context,
    ProxyClientStream,
};
use wrapper::{Cipher, WrapAddress, WrapSSTcp, WrapSSUdp};

#[derive(Debug, Deserialize, Clone, Config, JsonSchema)]
pub struct SSNetConfig {
    server: String,
    port: u16,
    password: String,
    #[serde(default)]
    udp: bool,

    cipher: Cipher,

    #[serde(default)]
    net: NetRef,
}

pub struct SSNet {
    context: Arc<Context>,
    config: SSNetConfig,
}

impl SSNet {
    fn new(config: SSNetConfig) -> SSNet {
        let context = Arc::new(Context::new(ServerType::Local));
        SSNet { context, config }
    }
}

#[async_trait]
impl INet for SSNet {
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: Address,
    ) -> Result<TcpStream> {
        let cfg = self.config.clone();
        let stream = self
            .config
            .net
            .tcp_connect(ctx, (cfg.server.as_ref(), cfg.port).into_address()?)
            .await?;
        let svr_cfg = ServerConfig::new((cfg.server, cfg.port), cfg.password, cfg.cipher.into());
        let client = ProxyClientStream::from_stream(
            self.context.clone(),
            stream,
            &svr_cfg,
            WrapAddress(addr),
        );
        Ok(WrapSSTcp(client).into_dyn())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        _addr: Address,
    ) -> Result<TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(&self, ctx: &mut rd_interface::Context, _addr: Address) -> Result<UdpSocket> {
        if !self.config.udp {
            return Err(NOT_ENABLED);
        }
        let cfg = self.config.clone();
        let socket = self
            .config
            .net
            .udp_bind(ctx, "0.0.0.0:0".into_address()?)
            .await?;
        let svr_cfg = ServerConfig::new((cfg.server, cfg.port), cfg.password, cfg.cipher.into());
        let udp = WrapSSUdp::new(self.context.clone(), socket, &svr_cfg);
        Ok(udp.into_dyn())
    }
}

impl NetFactory for SSNet {
    const NAME: &'static str = "shadowsocks";
    type Config = SSNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(SSNet::new(config))
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<SSNet>();

    Ok(())
}
