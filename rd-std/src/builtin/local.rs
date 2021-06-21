use std::{
    io::{self, ErrorKind},
    net::SocketAddr,
};

use rd_interface::{
    async_trait, impl_async_read_write, prelude::*, registry::NetFactory, Address, INet, IntoDyn,
    Result, TcpListener, TcpStream, UdpSocket,
};
use tokio::net;

#[rd_config]
#[derive(Debug, Clone, Default)]
pub struct LocalNetConfig {
    /// set ttl
    #[serde(default)]
    pub ttl: Option<u32>,

    /// set nodelay
    #[serde(default)]
    pub nodelay: Option<bool>,
}

pub struct LocalNet(LocalNetConfig);
pub struct CompatTcp(pub(crate) net::TcpStream);
pub struct Listener(net::TcpListener, LocalNetConfig);
pub struct Udp(net::UdpSocket);

impl LocalNet {
    pub fn new(config: LocalNetConfig) -> LocalNet {
        LocalNet(config)
    }
}

async fn lookup_host(domain: String, port: u16) -> io::Result<SocketAddr> {
    use tokio::net::lookup_host;

    let domain = (domain.as_ref(), port);
    lookup_host(domain)
        .await?
        .next()
        .ok_or(ErrorKind::AddrNotAvailable.into())
}

impl_async_read_write!(CompatTcp, 0);

#[async_trait]
impl rd_interface::ITcpStream for CompatTcp {
    async fn peer_addr(&self) -> Result<SocketAddr> {
        self.0.peer_addr().map_err(Into::into)
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().map_err(Into::into)
    }
}
impl CompatTcp {
    fn new(t: net::TcpStream) -> CompatTcp {
        CompatTcp(t)
    }
}

#[async_trait]
impl rd_interface::ITcpListener for Listener {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        let (socket, addr) = self.0.accept().await?;
        if let Some(ttl) = self.1.ttl {
            socket.set_ttl(ttl)?;
        }
        if let Some(nodelay) = self.1.nodelay {
            socket.set_nodelay(nodelay)?;
        }
        Ok((CompatTcp::new(socket).into_dyn(), addr))
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().map_err(Into::into)
    }
}

#[async_trait]
impl rd_interface::IUdpSocket for Udp {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        self.0.recv_from(buf).await.map_err(Into::into)
    }

    async fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize> {
        let addr = addr.resolve(lookup_host).await?;
        self.0.send_to(buf, addr).await.map_err(Into::into)
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().map_err(Into::into)
    }
}

#[async_trait]
impl INet for LocalNet {
    async fn tcp_connect(
        &self,
        _ctx: &mut rd_interface::Context,
        addr: Address,
    ) -> Result<TcpStream> {
        #[cfg(feature = "local_log")]
        tracing::trace!("local::tcp_connect {:?} {:?}", _ctx, addr);
        let addr = addr.resolve(lookup_host).await?;
        let tcp = net::TcpStream::connect(addr).await?;
        if let Some(ttl) = self.0.ttl {
            tcp.set_ttl(ttl)?;
        }
        if let Some(nodelay) = self.0.nodelay {
            tcp.set_nodelay(nodelay)?;
        }
        Ok(CompatTcp::new(tcp).into_dyn())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        addr: Address,
    ) -> Result<TcpListener> {
        #[cfg(feature = "local_log")]
        tracing::trace!("local::tcp_bind {:?} {:?}", _ctx, addr);
        let addr = addr.resolve(lookup_host).await?;
        let listener = net::TcpListener::bind(addr).await?;
        if let Some(ttl) = self.0.ttl {
            listener.set_ttl(ttl)?;
        }
        Ok(Listener(listener, self.0.clone()).into_dyn())
    }

    async fn udp_bind(&self, _ctx: &mut rd_interface::Context, addr: Address) -> Result<UdpSocket> {
        #[cfg(feature = "local_log")]
        tracing::trace!("local::udp_bind {:?} {:?}", _ctx, addr);
        let addr = addr.resolve(lookup_host).await?;
        let udp = net::UdpSocket::bind(addr).await?;
        if let Some(ttl) = self.0.ttl {
            udp.set_ttl(ttl)?;
        }
        Ok(Udp(udp).into_dyn())
    }
}

impl NetFactory for LocalNet {
    const NAME: &'static str = "local";
    type Config = LocalNetConfig;
    type Net = Self;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(LocalNet::new(config))
    }
}
