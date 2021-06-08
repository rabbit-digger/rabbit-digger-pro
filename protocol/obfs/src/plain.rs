use crate::Obfs;
use rd_interface::{
    schemars::{self, JsonSchema},
    Address, Config, Result, TcpStream,
};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Config, JsonSchema)]
pub struct Plain;

impl Obfs for Plain {
    fn tcp_connect(
        &self,
        tcp: TcpStream,
        _ctx: &mut rd_interface::Context,
        _addr: Address,
    ) -> Result<TcpStream> {
        Ok(tcp)
    }

    fn tcp_accept(&self, tcp: TcpStream, _addr: std::net::SocketAddr) -> Result<TcpStream> {
        Ok(tcp)
    }
}
