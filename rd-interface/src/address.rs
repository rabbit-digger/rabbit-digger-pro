use std::{
    io::{Error, ErrorKind, Result},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
};

/// Address can be IPv4, IPv6 address or a domain with port.
#[derive(Debug, PartialEq, Clone)]
pub enum Address {
    IPv4(SocketAddrV4),
    IPv6(SocketAddrV6),
    Domain(String, u16),
}

/// Converts to address value.
pub trait IntoAddress: Send {
    fn into_address(self) -> Result<Address>;
    fn into_socket_addr(self) -> Result<SocketAddr>
    where
        Self: Sized,
    {
        self.into_address()?.to_socket_addr()
    }
}

fn no_addr() -> Error {
    ErrorKind::AddrNotAvailable.into()
}

impl IntoAddress for &str {
    fn into_address(self) -> Result<Address> {
        let mut parts = self.rsplitn(2, ":");
        let domain = parts.next().ok_or_else(no_addr)?;
        let port: u16 = parts
            .next()
            .ok_or_else(no_addr)?
            .parse()
            .map_err(|_| no_addr())?;
        Ok(Address::Domain(domain.to_string(), port))
    }
}

impl IntoAddress for (&str, u16) {
    fn into_address(self) -> Result<Address> {
        Ok(Address::Domain(self.0.to_string(), self.1))
    }
}

impl IntoAddress for (String, u16) {
    fn into_address(self) -> Result<Address> {
        Ok(Address::Domain(self.0, self.1))
    }
}

impl IntoAddress for SocketAddr {
    fn into_address(self) -> Result<Address> {
        Ok(self.into())
    }
}

impl IntoAddress for Address {
    fn into_address(self) -> Result<Address> {
        Ok(self)
    }
}

impl From<SocketAddr> for Address {
    fn from(addr: SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(v4) => Address::IPv4(v4),
            SocketAddr::V6(v6) => Address::IPv6(v6),
        }
    }
}

impl Address {
    /// Converts to SocketAddr if Address can be convert to.
    /// Otherwise [AddrNotAvailable](std::io::ErrorKind::AddrNotAvailable) is returned.
    pub fn to_socket_addr(self) -> Result<SocketAddr> {
        match self {
            Address::IPv4(v4) => Ok(SocketAddr::V4(v4)),
            Address::IPv6(v6) => Ok(SocketAddr::V6(v6)),
            _ => Err(no_addr()),
        }
    }

    /// Resolve domain to SocketAddr using `f`.
    pub async fn resolve<Fut>(&self, f: impl FnOnce(String, u16) -> Fut) -> Result<SocketAddr>
    where
        Fut: std::future::Future<Output = Result<SocketAddr>>,
    {
        match self {
            Address::IPv4(v4) => Ok(SocketAddr::V4(*v4)),
            Address::IPv6(v6) => Ok(SocketAddr::V6(*v6)),
            Address::Domain(d, p) => f(d.clone(), *p).await,
        }
    }
}
