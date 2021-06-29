use super::config::AnyMatcher;
use super::matcher::{MatchContext, Matcher, MaybeAsync};

impl Matcher for AnyMatcher {
    fn match_rule(&self, _match_context: &MatchContext) -> MaybeAsync<bool> {
        true.into()
    }
}
