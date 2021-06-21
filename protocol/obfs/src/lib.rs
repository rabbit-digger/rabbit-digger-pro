use std::net::SocketAddr;

use obfs_net::{ObfsNet, ObfsNetConfig};
use rd_interface::{
    prelude::*, registry::NetFactory, Address, Context, Registry, Result, TcpStream,
};

mod http_simple;
mod obfs_net;
mod plain;

impl NetFactory for ObfsNet {
    const NAME: &'static str = "obfs";

    type Config = ObfsNetConfig;

    type Net = ObfsNet;

    fn new(config: Self::Config) -> Result<Self::Net> {
        ObfsNet::new(config)
    }
}

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<ObfsNet>();

    Ok(())
}

/// An obfs protocol used in front of a `TcpStream`
pub trait Obfs {
    /// Wrap the `TcpStream` with obfs request and response. Used by client.
    fn tcp_connect(&self, tcp: TcpStream, ctx: &mut Context, addr: Address) -> Result<TcpStream>;
    /// Wrap the `TcpStream` with obfs request and response. Used by server.
    fn tcp_accept(&self, tcp: TcpStream, addr: SocketAddr) -> Result<TcpStream>;
}

#[rd_config]
#[derive(Debug)]
#[serde(rename_all = "snake_case", tag = "obfs_type")]
pub enum ObfsType {
    HttpSimple(http_simple::HttpSimple),
    Plain(plain::Plain),
}

impl Default for ObfsType {
    fn default() -> Self {
        ObfsType::Plain(plain::Plain)
    }
}

impl Obfs for ObfsType {
    fn tcp_connect(&self, tcp: TcpStream, ctx: &mut Context, addr: Address) -> Result<TcpStream> {
        match self {
            ObfsType::HttpSimple(i) => i.tcp_connect(tcp, ctx, addr),
            ObfsType::Plain(i) => i.tcp_connect(tcp, ctx, addr),
        }
    }

    fn tcp_accept(&self, tcp: TcpStream, addr: SocketAddr) -> Result<TcpStream> {
        match self {
            ObfsType::HttpSimple(i) => i.tcp_accept(tcp, addr),
            ObfsType::Plain(i) => i.tcp_accept(tcp, addr),
        }
    }
}
