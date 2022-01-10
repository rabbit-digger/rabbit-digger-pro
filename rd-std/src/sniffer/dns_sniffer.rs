use std::{
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
};

use super::service::ReverseLookup;
use futures::{ready, Stream, StreamExt};
use rd_interface::{
    async_trait, context::common_field::DestDomain, impl_sink, Address, AddressDomain, BytesMut,
    Context, INet, IUdpSocket, IntoDyn, Net, Result, UdpSocket,
};

/// This net is used for reverse lookup.
///
/// When a UDP packet recv from port 53, the DNS response will be recorded in this net.
/// And the DNS response will be sent to the client.
/// The tcp_connect to recorded IP will be recovered to domain name.
/// If the domain name is in the cache, this net will add "DestDomain" to the context.
pub struct DNSSnifferNet {
    net: Net,
    rl: ReverseLookup,
}

impl DNSSnifferNet {
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
impl INet for DNSSnifferNet {
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

impl Stream for MitmUdp {
    type Item = std::io::Result<(BytesMut, SocketAddr)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        let (data, from_addr) = match ready!(this.0.poll_next_unpin(cx)) {
            Some(r) => r?,
            None => return Poll::Ready(None),
        };

        if from_addr.port() == 53 {
            let packet = &data[..];
            self.1.record_packet(packet);
        }

        Poll::Ready(Some(Ok((data, from_addr))))
    }
}

impl_sink!(MitmUdp, 0);

#[async_trait]
impl IUdpSocket for MitmUdp {
    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().await
    }
}
