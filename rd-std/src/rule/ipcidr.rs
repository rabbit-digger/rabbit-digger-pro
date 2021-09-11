use super::config::IpCidrMatcher;
use super::matcher::{MatchContext, Matcher, MaybeAsync};
use smoltcp::wire::IpAddress;

impl IpCidrMatcher {
    fn test(&self, address: impl Into<IpAddress>) -> bool {
        let address: IpAddress = address.into();
        self.ipcidr.0.contains_addr(&address)
    }
}

impl Matcher for IpCidrMatcher {
    fn match_rule(&self, match_context: &MatchContext) -> MaybeAsync<bool> {
        match match_context.get_socket_addr() {
            Some(addr) => self.test(addr.ip()),
            None => false,
        }
        .into()
    }
}
