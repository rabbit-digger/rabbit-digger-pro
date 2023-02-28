use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub enum PollFuture<T> {
    Ready(T),
    Future(Pin<Box<dyn Future<Output = T> + Send>>),
}

impl<T> PollFuture<T>
where
    T: Clone,
{
    pub fn new(fut: impl Future<Output = T> + Send + 'static) -> Self {
        PollFuture::Future(Box::pin(fut))
    }
    pub fn ready(val: T) -> Self {
        PollFuture::Ready(val)
    }
    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<T> {
        match self {
            PollFuture::Ready(val) => Poll::Ready(val.clone()),
            PollFuture::Future(fut) => {
                let val = futures::ready!(fut.as_mut().poll(cx));
                *self = PollFuture::Ready(val.clone());
                Poll::Ready(val)
            }
        }
    }
    pub fn is_ready(&self) -> bool {
        match self {
            PollFuture::Ready(_) => true,
            PollFuture::Future(_) => false,
        }
    }
}
