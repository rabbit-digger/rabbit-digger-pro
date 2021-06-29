use super::config::IpCidrMatcher;
use super::matcher::{MatchContext, Matcher, MaybeAsync};
use rd_interface::Address;
use smoltcp::wire::IpAddress;

impl IpCidrMatcher {
    fn test(&self, address: impl Into<IpAddress>) -> bool {
        let address: IpAddress = address.into();
        self.ipcidr.0.contains_addr(&address)
    }
}

impl Matcher for IpCidrMatcher {
    fn match_rule(&self, match_context: &MatchContext) -> MaybeAsync<bool> {
        let addr = match_context.address();
        match addr {
            Address::SocketAddr(addr) => self.test(addr.ip()),
            // if it's a domain, try to parse it to SocketAddr.
            Address::Domain(_, _) => match addr.to_socket_addr() {
                Ok(addr) => self.test(addr.ip()),
                Err(_) => false,
            },
        }
        .into()
    }
}
