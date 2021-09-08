use std::net::SocketAddr;

use super::service::ReverseLookup;
use rd_interface::{
    async_trait, Address, Context, INet, IUdpSocket, IntoDyn, Net, Result, UdpSocket,
};

pub struct DNSNet {
    net: Net,
    rl: ReverseLookup,
}

impl DNSNet {
    pub fn new(net: Net) -> Self {
        Self {
            net,
            rl: ReverseLookup::new(),
        }
    }
    fn reverse_lookup(&self, addr: &Address) -> Address {
        match addr {
            Address::SocketAddr(sa) => self
                .rl
                .reverse_lookup(sa.ip())
                .map(|name| {
                    let domain = Address::Domain(name, sa.port());
                    tracing::trace!(?domain, "recovered domain");
                    domain
                })
                .unwrap_or_else(|| addr.clone()),
            d => d.clone(),
        }
    }
}

#[async_trait]
impl INet for DNSNet {
    async fn tcp_connect(
        &self,
        ctx: &mut Context,
        addr: &Address,
    ) -> Result<rd_interface::TcpStream> {
        self.net.tcp_connect(ctx, &self.reverse_lookup(addr)).await
    }

    async fn tcp_bind(
        &self,
        ctx: &mut Context,
        addr: &Address,
    ) -> Result<rd_interface::TcpListener> {
        self.net.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        let udp = self.net.udp_bind(ctx, &self.reverse_lookup(addr)).await?;
        Ok(MitmUdp(udp, self.rl.clone()).into_dyn())
    }
}

struct MitmUdp(UdpSocket, ReverseLookup);

#[async_trait]
impl IUdpSocket for MitmUdp {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        let (size, from_addr) = self.0.recv_from(buf).await?;
        if from_addr.port() == 53 {
            let packet = &buf[..size];
            self.1.record_packet(packet);
        }
        Ok((size, from_addr))
    }

    async fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize> {
        self.0.send_to(buf, addr).await
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().await
    }
}
