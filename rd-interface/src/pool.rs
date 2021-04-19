use std::future::Future;

use futures_executor::ThreadPool;
use futures_util::task::{SpawnError, SpawnExt};

#[derive(Debug, Clone)]
pub struct ConnectionPool {
    pool: ThreadPool,
}

impl ConnectionPool {
    pub fn new() -> std::io::Result<ConnectionPool> {
        Ok(ConnectionPool {
            pool: ThreadPool::new()?,
        })
    }
    pub fn spawn<Fut>(&self, future: Fut) -> Result<(), SpawnError>
    where
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.pool.spawn(future)
    }
}
