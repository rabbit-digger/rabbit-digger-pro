use futures::{future::BoxFuture, Future, FutureExt};
use rd_interface::Address;
use std::{pin, task};

pub(super) enum MaybeAsync<T> {
    Sync {
        value: Option<T>,
    },
    #[allow(dead_code)]
    Async {
        future: BoxFuture<'static, T>,
    },
}

impl<T> From<T> for MaybeAsync<T> {
    fn from(value: T) -> Self {
        MaybeAsync::Sync { value: Some(value) }
    }
}

impl<T: Unpin> Future for MaybeAsync<T> {
    type Output = T;

    fn poll(self: pin::Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        match self.get_mut() {
            MaybeAsync::Sync { value } => {
                task::Poll::Ready(value.take().expect("Don't poll twice on MaybeAsync"))
            }
            MaybeAsync::Async { future } => future.poll_unpin(cx),
        }
    }
}

pub(super) trait Matcher: Send + Sync {
    fn match_rule(&self, match_context: &MatchContext) -> MaybeAsync<bool>;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct MatchContext {
    address: Address,
}

impl MatchContext {
    pub fn from_context_address(_ctx: &rd_interface::Context, addr: &Address) -> MatchContext {
        MatchContext {
            address: addr.clone(),
        }
    }
    pub fn address(&self) -> &Address {
        &self.address
    }
}
