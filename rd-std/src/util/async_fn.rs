use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{ready, Future, FutureExt};
use parking_lot::Mutex;
use pin_project_lite::pin_project;
use rd_interface::{async_trait, AsyncRead, AsyncWrite, ReadBuf};

pub struct PollAsyncFn<R> {
    fut: Option<Mutex<BoxFuture<R>>>,
}

impl<R> PollAsyncFn<R> {
    pub fn new() -> Self {
        Self { fut: None }
    }
    pub fn poll_next<F>(&mut self, cx: &mut Context<'_>, f: F) -> Poll<R>
    where
        F: Fn() -> BoxFuture<R>,
    {
        loop {
            match &mut self.fut {
                Some(fut) => {
                    let r = ready!(fut.get_mut().poll_unpin(cx));
                    self.fut = None;
                    return Poll::Ready(r);
                }
                None => self.fut = Some(Mutex::new(f())),
            }
        }
    }
}

#[async_trait]
pub trait AsyncFnRead {
    async fn read(&mut self, buf_size: usize) -> io::Result<Vec<u8>>;
}

#[async_trait]
pub trait AsyncFnWrite {
    async fn write(&mut self, buf: Vec<u8>) -> io::Result<usize>;
    async fn flush(&mut self) -> io::Result<()>;
    async fn shutdown(&mut self) -> io::Result<()>;
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pin_project! {
    pub struct AsyncFnIO<T> {
        inner: T,

        read_fut: PollAsyncFn< io::Result<Vec<u8>>>,
        write_fut: PollAsyncFn< io::Result<usize>>,
        flush_fut: PollAsyncFn< io::Result<()>>,
        shutdown_fut: PollAsyncFn<io::Result<()>>,
    }
}

impl<T> AsyncFnIO<T>
where
    T: AsyncFnRead + AsyncFnWrite + Clone + Unpin + Send + Sync + 'static,
{
    pub fn new(inner: T) -> Self {
        Self {
            inner,

            read_fut: PollAsyncFn::new(),
            write_fut: PollAsyncFn::new(),
            flush_fut: PollAsyncFn::new(),
            shutdown_fut: PollAsyncFn::new(),
        }
    }
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
    pub fn get_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> AsyncWrite for AsyncFnIO<T>
where
    T: AsyncFnWrite + Clone + Send + 'static,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let AsyncFnIO {
            inner, write_fut, ..
        } = &mut *self;
        write_fut.poll_next(cx, || {
            let mut inner = inner.clone();
            let buf = buf.to_vec();
            Box::pin(async move { inner.write(buf).await })
        })
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let AsyncFnIO {
            inner, flush_fut, ..
        } = &mut *self;
        flush_fut.poll_next(cx, || {
            let mut inner = inner.clone();
            Box::pin(async move { inner.flush().await })
        })
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        let AsyncFnIO {
            inner,
            shutdown_fut,
            ..
        } = &mut *self;
        shutdown_fut.poll_next(cx, || {
            let mut inner = inner.clone();
            Box::pin(async move { inner.shutdown().await })
        })
    }
}

impl<T> AsyncRead for AsyncFnIO<T>
where
    T: AsyncFnRead + Clone + Send + 'static,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        read_buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let AsyncFnIO {
            inner, read_fut, ..
        } = &mut *self;

        let buf = ready!(read_fut.poll_next(cx, || {
            let mut inner = inner.clone();
            let size = read_buf.remaining();
            Box::pin(async move { inner.read(size).await })
        }))?;

        read_buf
            .initialize_unfilled_to(buf.len())
            .copy_from_slice(&buf);
        read_buf.advance(buf.len());

        return Poll::Ready(Ok(()));
    }
}
