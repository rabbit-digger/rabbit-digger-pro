use rd_interface::{
    async_trait,
    registry::NetRef,
    schemars::{self, JsonSchema},
    Address, Config, Context, INet, Result, NOT_IMPLEMENTED,
};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Config, JsonSchema)]
pub struct ObfsNetConfig {
    #[serde(default)]
    net: NetRef,
}

pub struct ObfsNet(ObfsNetConfig);

impl ObfsNet {
    pub fn new(config: ObfsNetConfig) -> Result<Self> {
        Ok(ObfsNet(config))
    }
}

#[async_trait]
impl INet for ObfsNet {
    async fn tcp_connect(
        &self,
        _ctx: &mut Context,
        _addr: Address,
    ) -> Result<rd_interface::TcpStream> {
        Err(NOT_IMPLEMENTED)
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut Context,
        _addr: Address,
    ) -> Result<rd_interface::TcpListener> {
        Err(NOT_IMPLEMENTED)
    }

    async fn udp_bind(
        &self,
        _ctx: &mut Context,
        _addr: Address,
    ) -> Result<rd_interface::UdpSocket> {
        Err(NOT_IMPLEMENTED)
    }
}
