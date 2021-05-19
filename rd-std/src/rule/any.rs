use super::config::AnyMatcher;
use super::matcher::{Matcher, MaybeAsync};
use rd_interface::Address;

impl Matcher for AnyMatcher {
    fn match_rule(&self, _ctx: &rd_interface::Context, _addr: &Address) -> MaybeAsync<bool> {
        true.into()
    }
}
