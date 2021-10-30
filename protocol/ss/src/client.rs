use super::wrapper::{Cipher, WrapAddress, WrapSSTcp, WrapSSUdp};
use rd_interface::{
    async_trait, prelude::*, registry::NetRef, Address, INet, IntoDyn, Net, Result, TcpStream,
    UdpSocket, NOT_ENABLED,
};
use shadowsocks::{
    config::{ServerConfig, ServerType},
    context::{Context, SharedContext},
    ProxyClientStream,
};
use tokio::sync::OnceCell;

#[rd_config]
#[derive(Debug, Clone)]
pub struct SSNetConfig {
    server: Address,
    password: String,
    #[serde(default)]
    udp: bool,

    cipher: Cipher,

    #[serde(default)]
    net: NetRef,
}

pub struct SSNet {
    context: OnceCell<SharedContext>,
    cfg: ServerConfig,
    addr: Address,
    udp: bool,
    net: Net,
}

impl SSNet {
    pub fn new(config: SSNetConfig) -> SSNet {
        SSNet {
            context: OnceCell::new(),
            addr: config.server.clone(),
            cfg: ServerConfig::new(
                (config.server.host(), config.server.port()),
                config.password,
                config.cipher.into(),
            ),
            udp: config.udp,
            net: (*config.net).clone(),
        }
    }
    pub async fn context(&self) -> SharedContext {
        self.context
            .get_or_init(|| async { Context::new_shared(ServerType::Local) })
            .await
            .clone()
    }
}

#[async_trait]
impl INet for SSNet {
    async fn tcp_connect(
        &self,
        ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> Result<TcpStream> {
        let stream = self.net.tcp_connect(ctx, &self.addr).await?;
        let client = ProxyClientStream::from_stream(
            self.context().await,
            stream,
            &self.cfg,
            WrapAddress(addr.clone()),
        );
        Ok(WrapSSTcp(client).into_dyn())
    }

    // TODO: do something with bind address?
    async fn udp_bind(
        &self,
        ctx: &mut rd_interface::Context,
        _addr: &Address,
    ) -> Result<UdpSocket> {
        if !self.udp {
            return Err(NOT_ENABLED);
        }
        let socket = self
            .net
            .udp_bind(ctx, &self.addr.to_any_addr_port()?)
            .await?;
        let udp = WrapSSUdp::new(self.context().await, socket, &self.cfg);
        Ok(udp.into_dyn())
    }
}
