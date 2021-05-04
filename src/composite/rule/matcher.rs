use futures::{future::BoxFuture, Future, FutureExt};
use pin_project_lite::pin_project;
use std::{fmt, pin, task};

pin_project! {
    #[project = EnumProj]
    pub(super) enum MaybeAsync<T> {
        Sync{ value: Option<T> },
        Async{ future: BoxFuture<'static, T> },
    }
}

impl<T> From<T> for MaybeAsync<T> {
    fn from(value: T) -> Self {
        MaybeAsync::Sync { value: Some(value) }
    }
}

impl<T> Future for MaybeAsync<T> {
    type Output = T;

    fn poll(self: pin::Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        match self.project() {
            EnumProj::Sync { value } => {
                task::Poll::Ready(value.take().expect("Don't poll twice on MaybeAsync"))
            }
            EnumProj::Async { future } => future.poll_unpin(cx),
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
