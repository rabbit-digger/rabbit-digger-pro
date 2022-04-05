use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::{Future, Stream, StreamExt};
use pin_project_lite::pin_project;
use tokio::time::{sleep, Sleep};

pin_project! {
    #[derive(Debug)]
    pub struct DebounceStream<S, Item> {
        #[pin]
        inner: S,
        timer: Option<Pin<Box<Sleep>>>,
        item: Option<Item>,
        delay: Duration,
    }
}

pub trait DebounceStreamExt: Stream {
    fn debounce(self, duration: Duration) -> DebounceStream<Self, Self::Item>
    where
        Self: Sized,
    {
        DebounceStream {
            inner: self,
            timer: None,
            item: None,
            delay: duration,
        }
    }
}
impl<T: Stream> DebounceStreamExt for T {}

impl<S> Stream for DebounceStream<S, S::Item>
where
    S: Stream + Unpin,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        match this.inner.poll_next_unpin(cx) {
            Poll::Ready(r) => {
                *this.timer = Some(Box::pin(sleep(*this.delay)));
                *this.item = r;
            }
            Poll::Pending => {}
        };
        let poll_timer = this.timer.as_mut().map(|t| Future::poll(Pin::new(t), cx));
        if let Some(Poll::Ready(_)) = poll_timer {
            *this.timer = None;
            Poll::Ready(this.item.take())
        } else {
            Poll::Pending
        }
    }
}
