pub use connect_tcp::connect_tcp;
pub use connect_udp::connect_udp;
pub use net::{CombineNet, NotImplementedNet};
pub use peekable_tcpstream::PeekableTcpStream;

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

mod connect_tcp;
mod connect_udp;
mod net;
mod peekable_tcpstream;

/// Helper function for converting IPv4 mapped IPv6 address
///
/// This is the same as `Ipv6Addr::to_ipv4_mapped`, but it is still unstable in the current libstd
fn to_ipv4_mapped(ipv6: &Ipv6Addr) -> Option<Ipv4Addr> {
    match ipv6.octets() {
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, a, b, c, d] => Some(Ipv4Addr::new(a, b, c, d)),
        _ => None,
    }
}

pub fn resolve_mapped_socket_addr(addr: SocketAddr) -> SocketAddr {
    if let SocketAddr::V6(ref a) = addr {
        if let Some(v4) = to_ipv4_mapped(a.ip()) {
            return SocketAddr::new(v4.into(), a.port());
        }
    }

    return addr;
}
