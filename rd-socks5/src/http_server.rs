use super::common::{pack_udp, parse_udp, Address};
use crate::protocol::{
    self, AuthMethod, AuthRequest, AuthResponse, CommandRequest, CommandResponse, Version,
};
use futures::{io::BufWriter, prelude::*};
use protocol::Command;
use rd_interface::{
    async_trait, pool::IUdpChannel, util::connect_tcp, ConnectionPool, Context, IntoAddress,
    IntoDyn, Net, Result, TcpStream, UdpSocket,
};
use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4},
    sync::{Arc, RwLock},
};

#[derive(Clone)]
pub struct HttpServer {
    net: Net,
}

impl HttpServer {
    pub async fn serve_connection(
        self,
        socket: TcpStream,
        addr: SocketAddr,
        pool: ConnectionPool,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    pub fn new(net: Net) -> Self {
        Self { net }
    }
}
