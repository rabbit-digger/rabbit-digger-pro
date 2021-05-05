mod udp;
mod wrapper;

use rd_interface::{
    async_trait,
    config::{from_value, Value},
    util::get_one_net,
    Address, Arc, INet, IntoAddress, IntoDyn, Net, Registry, Result, TcpListener, TcpStream,
    UdpSocket, NOT_IMPLEMENTED,
};
use serde_derive::Deserialize;
use shadowsocks::{
    config::{ServerConfig, ServerType},
    context::Context,
    ProxyClientStream,
};
use tokio_util::compat::*;
use wrapper::{WrapAddress, WrapCipher, WrapSSTcp, WrapSSUdp};

#[derive(Debug, Deserialize, Clone)]
struct SSNetConfig {
    server: String,
    port: u16,
    password: String,
    #[serde(deserialize_with = "crate::wrapper::deserialize_cipher")]
    cipher: WrapCipher,
}

pub struct SSNet {
    net: Net,
    context: Arc<Context>,
    config: SSNetConfig,
}

impl SSNet {
    fn new(net: Net, config: Value) -> rd_interface::Result<SSNet> {
        let context = Arc::new(Context::new(ServerType::Local));
        let config: SSNetConfig = from_value(config)?;
        Ok(SSNet {
            net,
            context,
            config,
        })
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
            .net
            .tcp_connect(ctx, (cfg.server.as_ref(), cfg.port).into_address()?)
            .await?
            .compat();
        let svr_cfg = ServerConfig::new((cfg.server, cfg.port), cfg.password, cfg.cipher.into());
        let client = ProxyClientStream::from_stream(
            self.context.clone(),
            stream,
            &svr_cfg,
            WrapAddress(addr),
        )
        .compat();
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
        let cfg = self.config.clone();
        let socket = self.net.udp_bind(ctx, "0.0.0.0:0".into_address()?).await?;
        let svr_cfg = ServerConfig::new((cfg.server, cfg.port), cfg.password, cfg.cipher.into());
        let udp = WrapSSUdp::new(self.context.clone(), socket, &svr_cfg);
        Ok(udp.into_dyn())
    }
}

#[no_mangle]
pub fn init_plugin(registry: &mut Registry) -> Result<()> {
    registry.add_net("shadowsocks", |nets, config| {
        SSNet::new(get_one_net(nets)?, config)
    });

    Ok(())
}
