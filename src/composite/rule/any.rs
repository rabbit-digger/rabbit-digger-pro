use std::fmt;

use super::matcher::{Matcher, MaybeAsync};
use rd_interface::Address;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AnyMatcher {}

impl AnyMatcher {
    pub fn new() -> AnyMatcher {
        AnyMatcher {}
    }
}

impl fmt::Display for AnyMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "any")
    }
}

impl Matcher for AnyMatcher {
    fn match_rule(&self, _ctx: &rd_interface::Context, _addr: &Address) -> MaybeAsync<bool> {
        true.into()
    }
}
