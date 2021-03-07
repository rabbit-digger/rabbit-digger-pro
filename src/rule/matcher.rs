use anyhow::{anyhow, Result};
use core::panic;
use futures::{future::BoxFuture, Future, FutureExt};
use pin_project_lite::pin_project;
use std::{collections::HashMap, pin, task};

pin_project! {
    #[project = EnumProj]
    pub(super) enum MaybeAsync<T> {
        Sync{ value: T },
        Async{ future: BoxFuture<'static, T> },
    }
}

impl<T> From<T> for MaybeAsync<T> {
    fn from(value: T) -> Self {
        MaybeAsync::Sync { value }
    }
}

impl<T: Copy> Future for MaybeAsync<T> {
    type Output = T;

    fn poll(self: pin::Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        match self.project() {
            EnumProj::Sync { value } => task::Poll::Ready(*value),
            EnumProj::Async { future } => future.poll_unpin(cx),
        }
    }
}

pub(super) trait Matcher: Send + Sync {
    fn match_rule(
        &self,
        ctx: &rd_interface::Context,
        addr: &rd_interface::Address,
    ) -> MaybeAsync<bool>;
}
pub(super) type BoxMatcher = Box<dyn Matcher>;

pub(super) type MatcherFactory = Box<dyn Fn(rd_interface::config::Value) -> Result<BoxMatcher>>;
pub struct MatcherRegistry(HashMap<String, MatcherFactory>);

impl MatcherRegistry {
    pub fn new() -> MatcherRegistry {
        MatcherRegistry(HashMap::new())
    }
    pub(super) fn register(
        &mut self,
        name: impl Into<String>,
        factory: impl Fn(rd_interface::config::Value) -> Result<BoxMatcher> + 'static,
    ) {
        match self.0.insert(name.into(), Box::new(factory)) {
            Some(_) => panic!("duplicate matcher"),
            None => {}
        }
    }
    pub(super) fn get(
        &self,
        name: impl AsRef<str>,
        value: rd_interface::config::Value,
    ) -> Result<BoxMatcher> {
        let name = name.as_ref();
        self.0
            .get(name)
            .map(move |f| f(value))
            .transpose()?
            .ok_or(anyhow!("Rule type is not supported: {}", name))
    }
}
