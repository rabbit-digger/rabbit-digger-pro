use serde::{de, ser};
use std::{
    fmt,
    io::{Error, ErrorKind, Result},
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

/// Address can be IPv4, IPv6 address or a domain with port.
#[derive(Debug, PartialEq, Clone, PartialOrd, Eq, Ord)]
pub enum Address {
    SocketAddr(SocketAddr),
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

fn strip_brackets(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host)
}

fn host_to_address(host: &str, port: u16) -> Address {
    match strip_brackets(host).parse::<IpAddr>() {
        Ok(ip) => {
            let addr = SocketAddr::new(ip, port);
            addr.into()
        }
        Err(_) => Address::Domain(host.to_string(), port),
    }
}

impl IntoAddress for &str {
    fn into_address(self) -> Result<Address> {
        let mut parts = self.rsplitn(2, ':');
        let port: u16 = parts
            .next()
            .ok_or_else(no_addr)?
            .parse()
            .map_err(|_| no_addr())?;
        let host = parts.next().ok_or_else(no_addr)?;
        Ok(host_to_address(host, port))
    }
}

impl FromStr for Address {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        IntoAddress::into_address(s)
    }
}

impl IntoAddress for (&str, u16) {
    fn into_address(self) -> Result<Address> {
        Ok(host_to_address(self.0, self.1))
    }
}

impl IntoAddress for (String, u16) {
    fn into_address(self) -> Result<Address> {
        Ok(host_to_address(&self.0, self.1))
    }
}

impl IntoAddress for (IpAddr, u16) {
    fn into_address(self) -> Result<Address> {
        let addr = SocketAddr::new(self.0, self.1);
        Ok(addr.into())
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
        Address::SocketAddr(addr)
    }
}

impl From<(IpAddr, u16)> for Address {
    fn from((ip, port): (IpAddr, u16)) -> Self {
        Address::SocketAddr(SocketAddr::new(ip, port))
    }
}

impl Address {
    /// Converts to SocketAddr if Address can be convert to.
    /// Otherwise [AddrNotAvailable](std::io::ErrorKind::AddrNotAvailable) is returned.
    pub fn to_socket_addr(&self) -> Result<SocketAddr> {
        match self {
            Address::SocketAddr(s) => Ok(*s),
            Address::Domain(d, p) => match strip_brackets(d).parse::<IpAddr>() {
                Ok(ip) => Ok(SocketAddr::new(ip, *p)),
                Err(_) => Err(no_addr()),
            },
        }
    }

    /// Resolve domain to SocketAddr using `f`.
    pub async fn resolve<Fut>(&self, f: impl FnOnce(String, u16) -> Fut) -> Result<SocketAddr>
    where
        Fut: std::future::Future<Output = Result<SocketAddr>>,
    {
        match self {
            Address::SocketAddr(s) => Ok(*s),
            Address::Domain(d, p) => match strip_brackets(d).parse::<IpAddr>() {
                Ok(ip) => Ok(SocketAddr::new(ip, *p)),
                Err(_) => f(d.to_string(), *p).await,
            },
        }
    }

    /// Get host part of the Address
    pub fn host(&self) -> String {
        match self {
            Address::SocketAddr(s) => s.ip().to_string(),
            Address::Domain(d, _) => d.to_string(),
        }
    }

    /// Get port of the Address
    pub fn port(&self) -> u16 {
        match self {
            Address::SocketAddr(s) => s.port(),
            Address::Domain(_, p) => *p,
        }
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Address::Domain(domain, port) => write!(f, "{}:{}", domain, port),
            Address::SocketAddr(s) => write!(f, "{}", s),
        }
    }
}

impl<'de> de::Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl ser::Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    const IPV4_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
    const IPV6_ADDR: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
    const DOMAIN: &'static str = "example.com";

    #[test]
    fn test_address_convert() {
        let ipv4_addr = Address::SocketAddr(SocketAddr::new(IPV4_ADDR, 1234));
        let ipv6_addr = Address::SocketAddr(SocketAddr::new(IPV6_ADDR, 1234));
        let domain_addr = Address::Domain(DOMAIN.to_string(), 1234);

        // &str
        assert_eq!(ipv4_addr, "1.2.3.4:1234".into_address().unwrap());
        assert_eq!(ipv6_addr, "[1:2:3:4:5:6:7:8]:1234".into_address().unwrap());
        assert_eq!(domain_addr, "example.com:1234".into_address().unwrap());

        // (&str, u16)
        assert_eq!(ipv4_addr, ("1.2.3.4", 1234).into_address().unwrap());
        assert_eq!(
            ipv6_addr,
            ("[1:2:3:4:5:6:7:8]", 1234).into_address().unwrap()
        );
        assert_eq!(domain_addr, ("example.com", 1234).into_address().unwrap());

        // (String, u16)
        assert_eq!(
            ipv4_addr,
            ("1.2.3.4".to_string(), 1234).into_address().unwrap()
        );
        assert_eq!(
            ipv6_addr,
            ("[1:2:3:4:5:6:7:8]".to_string(), 1234)
                .into_address()
                .unwrap()
        );
        assert_eq!(
            domain_addr,
            ("example.com".to_string(), 1234).into_address().unwrap()
        );

        // (IpAddr, u16)
        assert_eq!(ipv4_addr, (IPV4_ADDR, 1234).into_address().unwrap());
        assert_eq!(ipv6_addr, (IPV6_ADDR, 1234).into_address().unwrap());
    }
}
