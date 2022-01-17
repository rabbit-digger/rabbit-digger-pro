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

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use futures::SinkExt;
    use rd_interface::{Bytes, IntoAddress};

    use crate::tests::TestNet;

    use super::*;

    #[tokio::test]
    async fn test_dns_sniffer() {
        let test_net = TestNet::new().into_dyn();
        let net = DNSSnifferNet::new(test_net.clone());

        let mut ctx = Context::new();
        let mut dns_server = net
            .udp_bind(&mut ctx, &"127.0.0.1:53".into_address().unwrap())
            .await
            .unwrap();
        let mut client = net
            .udp_bind(&mut ctx, &"127.0.0.1:0".into_address().unwrap())
            .await
            .unwrap();

        // dns request to baidu.com
        client
            .send((
                Bytes::from_static(&[
                    0x00, 0x02, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
                    0x62, 0x61, 0x69, 0x64, 0x75, 0x03, 0x63, 0x6F, 0x6D, 0x00, 0x00, 0x01, 0x00,
                    0x01,
                ]),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 53).into(),
            ))
            .await
            .unwrap();
        let (_, addr) = dns_server.next().await.unwrap().unwrap();

        assert_eq!(addr, client.local_addr().await.unwrap());

        // dns response to baidu.com. 220.181.38.148, 220.181.38.251
        dns_server
            .send((
                Bytes::from_static(&[
                    0x00, 0x02, 0x81, 0x80, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x05,
                    0x62, 0x61, 0x69, 0x64, 0x75, 0x03, 0x63, 0x6F, 0x6D, 0x00, 0x00, 0x01, 0x00,
                    0x01, 0xC0, 0x0C, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x01, 0xFA, 0x00, 0x04,
                    0xDC, 0xB5, 0x26, 0x94, 0xC0, 0x0C, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x01,
                    0xFA, 0x00, 0x04, 0xDC, 0xB5, 0x26, 0xFB,
                ]),
                addr.into(),
            ))
            .await
            .unwrap();
        let _ = client.next().await.unwrap().unwrap();

        assert_eq!(
            net.rl
                .reverse_lookup(Ipv4Addr::new(220, 181, 38, 148).into()),
            Some("baidu.com".to_string()),
        );
    }
}
