use std::{
    ops::{Deref, DerefMut},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::FutureExt;
use lru_time_cache::LruCache as LruTimeCache;
use tokio::time::{sleep, Instant, Sleep};

/// A LruCache with a time-to-live expiration.
pub struct LruCache<Key, Value> {
    cache: LruTimeCache<Key, Value>,
    ttl: Duration,
    sleep: Pin<Box<Sleep>>,
}

impl<Key, Value> LruCache<Key, Value>
where
    Key: Ord + Clone,
{
    /// Constructor for time based `LruCache`.
    pub fn with_expiry_duration(time_to_live: Duration) -> LruCache<Key, Value> {
        LruCache {
            cache: LruTimeCache::with_expiry_duration(time_to_live),
            ttl: time_to_live,
            sleep: Box::pin(sleep(time_to_live)),
        }
    }

    /// Constructor for dual-feature capacity and time based `LruCache`.
    pub fn with_expiry_duration_and_capacity(
        time_to_live: Duration,
        capacity: usize,
    ) -> LruCache<Key, Value> {
        LruCache {
            cache: LruTimeCache::with_expiry_duration_and_capacity(time_to_live, capacity),
            ttl: time_to_live,
            sleep: Box::pin(sleep(time_to_live)),
        }
    }

    pub fn poll_clear_expired(&mut self, cx: &mut Context<'_>) {
        loop {
            match self.sleep.poll_unpin(cx) {
                Poll::Pending => return,
                Poll::Ready(_) => {
                    self.sleep.as_mut().reset(Instant::now() + self.ttl);

                    // removes expired connections
                    self.cache.iter();
                }
            }
        }
    }
}

impl<Key, Value> Deref for LruCache<Key, Value> {
    type Target = LruTimeCache<Key, Value>;

    fn deref(&self) -> &Self::Target {
        &self.cache
    }
}

impl<Key, Value> DerefMut for LruCache<Key, Value> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cache
    }
}
