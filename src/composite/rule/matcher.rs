use futures::{future::BoxFuture, Future, FutureExt};
use std::{fmt, pin, task};

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

pub(super) trait Matcher: Send + Sync + fmt::Display {
    fn match_rule(
        &self,
        ctx: &rd_interface::Context,
        addr: &rd_interface::Address,
    ) -> MaybeAsync<bool>;
}
pub(super) type BoxMatcher = Box<dyn Matcher>;
