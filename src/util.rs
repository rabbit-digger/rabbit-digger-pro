use std::{
    collections::HashMap,
    hash::Hash,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use async_std::stream;
use futures::{Stream, StreamExt};
use pin_project_lite::pin_project;
use topological_sort::TopologicalSort;

pub fn topological_sort<K, V, D>(mut map: HashMap<K, V>, get_deps: D) -> Option<Vec<(K, V)>>
where
    K: Hash + Eq + Clone,
    D: Fn(&V) -> Vec<&K>,
{
    let mut ts = TopologicalSort::<&K>::new();

    for (k, v) in map.iter() {
        for d in get_deps(v).into_iter() {
            ts.add_dependency(d, k);
        }
    }

    let mut list = Vec::<K>::new();
    while let Some(k) = ts.pop() {
        list.push(k.clone());
    }

    if ts.len() > 0 {
        return None;
    }

    Some(
        list.into_iter()
            .map(|k| {
                let v = map.remove(&k);
                v.map(|v| (k, v))
            })
            .filter_map(|i| i)
            .collect(),
    )
}

type Timer = stream::Timeout<stream::Pending<()>>;

fn timer(duration: Duration) -> Timer {
    use async_std::stream::StreamExt;
    stream::pending().timeout(duration)
}

pin_project! {
    #[derive(Debug)]
    pub struct DebounceStream<S, Item> {
        #[pin]
        inner: S,
        timer: Option<Timer>,
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
                *this.timer = Some(timer(*this.delay));
                *this.item = r;
            }
            Poll::Pending => {}
        };
        let poll_timer = this.timer.as_mut().map(|t| t.poll_next_unpin(cx));
        if let Some(Poll::Ready(Some(_))) = poll_timer {
            *this.timer = None;
            Poll::Ready(this.item.take())
        } else {
            Poll::Pending
        }
    }
}
