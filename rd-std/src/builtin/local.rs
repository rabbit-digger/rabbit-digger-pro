use std::{
    io,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use rd_interface::{
    async_trait, impl_async_read_write, prelude::*, registry::NetFactory, Address, INet, IntoDyn,
    Result, TcpListener, TcpStream, UdpSocket,
};
use socket2::{Domain, Socket, Type};
use tokio::{net, time::timeout};
use tracing::instrument;

#[rd_config]
#[derive(Debug, Clone, Default)]
pub struct LocalNetConfig {
    /// set ttl
    #[serde(default)]
    pub ttl: Option<u32>,

    /// set nodelay
    #[serde(default)]
    pub nodelay: Option<bool>,

    /// set SO_MARK on linux
    pub mark: Option<u32>,

    /// bind to device
    pub bind_device: Option<String>,

    /// bind to address
    pub bind_addr: Option<IpAddr>,

    /// timeout of TCP connect, in seconds.
    pub connect_timeout: Option<u64>,
}

pub struct LocalNet(LocalNetConfig);
pub struct CompatTcp(pub(crate) net::TcpStream);
pub struct Listener(net::TcpListener, LocalNetConfig);
pub struct Udp(net::UdpSocket);

impl LocalNet {
    pub fn new(config: LocalNetConfig) -> LocalNet {
        LocalNet(config)
    }
    fn set_socket(&self, socket: &Socket, _addr: SocketAddr) -> Result<()> {
        socket.set_nonblocking(true)?;

        if let Some(local_addr) = self.0.bind_addr {
            socket.bind(&SocketAddr::new(local_addr, 0).into())?;
        }

        if let Some(ttl) = self.0.ttl {
            socket.set_ttl(ttl)?;
        }

        if let Some(nodelay) = self.0.nodelay {
            socket.set_nodelay(nodelay)?;
        }

        #[cfg(target_os = "linux")]
        if let Some(mark) = self.0.mark {
            socket.set_mark(mark)?;
        }

        #[cfg(target_os = "linux")]
        if let Some(device) = &self.0.bind_device {
            socket.bind_device(Some(device.as_bytes()))?;
        }

        #[cfg(target_os = "macos")]
        if let Some(device) = &self.0.bind_device {
            let device = std::ffi::CString::new(device.as_bytes())
                .map_err(rd_interface::error::map_other)?;
            unsafe {
                let idx = libc::if_nametoindex(device.as_ptr());
                if idx == 0 {
                    return Err(io::Error::last_os_error().into());
                }

                const IPV6_BOUND_IF: libc::c_int = 125;
                let ret = match _addr {
                    SocketAddr::V4(_) => libc::setsockopt(
                        std::os::unix::prelude::AsRawFd::as_raw_fd(socket),
                        libc::IPPROTO_IP,
                        libc::IP_BOUND_IF,
                        &idx as *const _ as *const libc::c_void,
                        std::mem::size_of::<libc::c_uint>() as libc::socklen_t,
                    ),
                    SocketAddr::V6(_) => libc::setsockopt(
                        std::os::unix::prelude::AsRawFd::as_raw_fd(socket),
                        libc::IPPROTO_IPV6,
                        IPV6_BOUND_IF,
                        &idx as *const _ as *const libc::c_void,
                        std::mem::size_of::<libc::c_uint>() as libc::socklen_t,
                    ),
                };

                if ret == -1 {
                    return Err(io::Error::last_os_error().into());
                }
            }
        }

        Ok(())
    }
    async fn tcp_connect_single(&self, addr: SocketAddr) -> Result<net::TcpStream> {
        let socket = match addr {
            SocketAddr::V4(_) => Socket::new(Domain::IPV4, Type::STREAM, None)?,
            SocketAddr::V6(_) => Socket::new(Domain::IPV6, Type::STREAM, None)?,
        };

        self.set_socket(&socket, addr)?;

        let socket = net::TcpSocket::from_std_stream(socket.into());

        let tcp = match self.0.connect_timeout {
            None => socket.connect(addr).await?,
            Some(secs) => timeout(Duration::from_secs(secs), socket.connect(addr)).await??,
        };

        Ok(tcp)
    }
    async fn tcp_bind_single(&self, addr: SocketAddr) -> Result<net::TcpListener> {
        let listener = net::TcpListener::bind(addr).await?;

        Ok(listener)
    }
    async fn udp_bind_single(&self, addr: SocketAddr) -> Result<net::UdpSocket> {
        let udp = match addr {
            SocketAddr::V4(_) => Socket::new(Domain::IPV4, Type::DGRAM, None)?,
            SocketAddr::V6(_) => Socket::new(Domain::IPV6, Type::DGRAM, None)?,
        };

        self.set_socket(&udp, addr)?;

        udp.bind(&addr.into())?;

        let udp = net::UdpSocket::from_std(udp.into())?;

        Ok(udp)
    }
}

impl std::fmt::Debug for LocalNet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalNet").finish()
    }
}

#[instrument(err)]
async fn lookup_host(domain: String, port: u16) -> io::Result<Vec<SocketAddr>> {
    use tokio::net::lookup_host;

    let domain = (domain.as_ref(), port);
    Ok(lookup_host(domain).await?.collect())
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

impl Udp {
    async fn send_to_single(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        self.0.send_to(buf, addr).await.map_err(Into::into)
    }
}

#[async_trait]
impl rd_interface::IUdpSocket for Udp {
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        self.0.recv_from(buf).await.map_err(Into::into)
    }

    async fn send_to(&self, buf: &[u8], addr: Address) -> Result<usize> {
        let addrs = addr.resolve(lookup_host).await?;

        match addrs.into_iter().next() {
            Some(target) => self.send_to_single(buf, target).await,
            None => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no addresses to send data to",
            )
            .into()),
        }
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr().map_err(Into::into)
    }
}

#[async_trait]
impl INet for LocalNet {
    #[instrument(err)]
    async fn tcp_connect(
        &self,
        _ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> Result<TcpStream> {
        let addrs = addr.resolve(lookup_host).await?;
        let mut last_err = None;

        for addr in addrs {
            match self.tcp_connect_single(addr).await {
                Ok(stream) => return Ok(CompatTcp::new(stream).into_dyn()),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
            .into()
        }))
    }

    #[instrument(err)]
    async fn tcp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> Result<TcpListener> {
        let addrs = addr.resolve(lookup_host).await?;
        let mut last_err = None;

        for addr in addrs {
            match self.tcp_bind_single(addr).await {
                Ok(listener) => return Ok(Listener(listener, self.0.clone()).into_dyn()),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
            .into()
        }))
    }

    #[instrument(err)]
    async fn udp_bind(
        &self,
        _ctx: &mut rd_interface::Context,
        addr: &Address,
    ) -> Result<UdpSocket> {
        let addrs = addr.resolve(lookup_host).await?;
        let mut last_err = None;

        for addr in addrs {
            match self.udp_bind_single(addr).await {
                Ok(udp) => return Ok(Udp(udp).into_dyn()),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
            .into()
        }))
    }

    #[instrument(err)]
    async fn lookup_host(&self, addr: &Address) -> Result<Vec<SocketAddr>> {
        let addr = addr.resolve(lookup_host).await?;
        Ok(addr)
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
