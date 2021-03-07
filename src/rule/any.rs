use super::matcher::{Matcher, MatcherRegistry, MaybeAsync};
use rd_interface::{config::from_value, Address};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AnyMatcher {}

impl AnyMatcher {
    pub fn register(registry: &mut MatcherRegistry) {
        registry.register("any", |value| {
            Ok(Box::new(from_value::<AnyMatcher>(value)?))
        });
    }
}

impl Matcher for AnyMatcher {
    fn match_rule(&self, _ctx: &rd_interface::Context, _addr: &Address) -> MaybeAsync<bool> {
        true.into()
    }
}
