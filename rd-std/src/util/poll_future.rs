use std::{
    future::Future,
    mem::replace,
    pin::Pin,
    task::{Context, Poll},
};

pub enum PollFuture<T> {
    Empty,
    Ready(T),
    Future(Pin<Box<dyn Future<Output = T> + Send>>),
}

impl<T> PollFuture<T> {
    pub fn new(fut: impl Future<Output = T> + Send + 'static) -> Self {
        PollFuture::Future(Box::pin(fut))
    }
    pub fn ready(val: T) -> Self {
        PollFuture::Ready(val)
    }
    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<T> {
        match self {
            PollFuture::Empty => panic!("PollFuture::poll called on empty future"),
            PollFuture::Ready(_) => {
                let val = replace(self, PollFuture::Empty);
                Poll::Ready(match val {
                    PollFuture::Ready(val) => val,
                    _ => unreachable!(),
                })
            }
            PollFuture::Future(fut) => {
                let val = futures::ready!(fut.as_mut().poll(cx));
                *self = PollFuture::Empty;
                Poll::Ready(val)
            }
        }
    }
    pub fn is_ready(&self) -> bool {
        match self {
            PollFuture::Empty => true,
            PollFuture::Ready(_) => true,
            PollFuture::Future(_) => false,
        }
    }
}
