use std::net::SocketAddr;

use super::service::ReverseLookup;
use rd_interface::{
    async_trait, context::common_field::DestDomain, Address, AddressDomain, Context, INet,
    IUdpSocket, IntoDyn, Net, Result, UdpSocket,
};

/// This net is used for reverse lookup.
///
/// When a UDP packet recv from port 53, the DNS response will be recorded in this net.
/// And the DNS response will be sent to the client.
/// The tcp_connect to recorded IP will be recovered to domain name.
/// If the domain name is in the cache, this net will add "DestDomain" to the context.
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
    fn reverse_lookup(&self, ctx: &mut Context, addr: &Address) -> Address {
        match addr {
            Address::SocketAddr(sa) => self
                .rl
                .reverse_lookup(sa.ip())
                .map(|name| {
                    let domain = Address::Domain(name.clone(), sa.port());
                    ctx.insert_common(DestDomain(AddressDomain {
                        domain: name,
                        port: sa.port(),
                    }))
                    .expect("Failed to insert domain");
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
        let addr = &self.reverse_lookup(ctx, addr);
        self.net.tcp_connect(ctx, addr).await
    }

    async fn tcp_bind(
        &self,
        ctx: &mut Context,
        addr: &Address,
    ) -> Result<rd_interface::TcpListener> {
        self.net.tcp_bind(ctx, addr).await
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<UdpSocket> {
        let udp = self.net.udp_bind(ctx, addr).await?;
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
