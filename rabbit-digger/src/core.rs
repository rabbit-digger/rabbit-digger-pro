use std::{collections::HashMap, io};

use apir::{
    prelude::*,
    traits::{async_trait, BoxedTcpStream},
};

pub enum Match {}

pub struct Rule {
    output: String,
}

pub struct RuleSet {
    output: HashMap<String, BoxedTcpStream>,
    rules: Vec<Rule>,
}

impl RuleSet {
    pub fn new() -> Self {
        RuleSet {
            output: HashMap::new(),
            rules: vec![],
        }
    }
}

#[async_trait]
impl ProxyTcpStream for RuleSet {
    type TcpStream = BoxedTcpStream;

    async fn tcp_connect<A: IntoAddress>(&self, addr: A) -> io::Result<Self::TcpStream> {
        let addr = addr.into_address()?;
        todo!()
    }
}

#[async_trait]
impl ProxyUdpSocket for RuleSet {
    type UdpSocket = BoxedUdpSocket;

    async fn udp_bind<A: IntoAddress>(&self, addr: A) -> io::Result<Self::UdpSocket> {
        let addr = addr.into_address()?;
        todo!()
    }
}
