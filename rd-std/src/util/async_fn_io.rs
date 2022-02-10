use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{ready, Future, FutureExt};
use parking_lot::Mutex;
use pin_project_lite::pin_project;
use rd_interface::{async_trait, AsyncRead, AsyncWrite, ReadBuf};

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

type BoxFuture<T> = Mutex<Pin<Box<dyn Future<Output = T> + Send + 'static>>>;

pin_project! {
    pub struct AsyncFnIO<T> {
        inner: T,

        read_fut: Option<BoxFuture<io::Result<Vec<u8>>>>,
        write_fut: Option<BoxFuture<io::Result<usize>>>,
        flush_fut: Option<BoxFuture<io::Result<()>>>,
        shutdown_fut: Option<BoxFuture<io::Result<()>>>,
    }
}

impl<T> AsyncFnIO<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,

            read_fut: None,
            write_fut: None,
            flush_fut: None,
            shutdown_fut: None,
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
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.project();
        let write_fut = this.write_fut;

        loop {
            match write_fut {
                Some(fut) => {
                    let size = ready!(fut.lock().poll_unpin(cx))?;

                    *write_fut = None;
                    return Poll::Ready(Ok(size));
                }
                None => {
                    let mut inner = this.inner.clone();
                    let buf = buf.to_vec();
                    let fut = async move { inner.write(buf).await };
                    *write_fut = Some(Mutex::new(Box::pin(fut)));
                }
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        let flush_fut = this.flush_fut;

        loop {
            match flush_fut {
                Some(fut) => {
                    let _ = ready!(fut.lock().poll_unpin(cx))?;

                    *flush_fut = None;
                    return Poll::Ready(Ok(()));
                }
                None => {
                    let mut inner = this.inner.clone();
                    let fut = async move { inner.flush().await };
                    *flush_fut = Some(Mutex::new(Box::pin(fut)));
                }
            }
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        let shutdown_fut = this.shutdown_fut;

        loop {
            match shutdown_fut {
                Some(fut) => {
                    let _ = ready!(fut.lock().poll_unpin(cx))?;

                    *shutdown_fut = None;
                    return Poll::Ready(Ok(()));
                }
                None => {
                    let mut inner = this.inner.clone();
                    let fut = async move { inner.shutdown().await };
                    *shutdown_fut = Some(Mutex::new(Box::pin(fut)));
                }
            }
        }
    }
}

impl<T> AsyncRead for AsyncFnIO<T>
where
    T: AsyncFnRead + Clone + Send + 'static,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        read_buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        let read_fut = this.read_fut;

        loop {
            match read_fut {
                Some(fut) => {
                    let buf = ready!(fut.lock().poll_unpin(cx))?;

                    read_buf
                        .initialize_unfilled_to(buf.len())
                        .copy_from_slice(&buf);
                    read_buf.advance(buf.len());

                    *read_fut = None;
                    return Poll::Ready(Ok(()));
                }
                None => {
                    let mut inner = this.inner.clone();
                    let size = read_buf.initialize_unfilled().len();
                    let fut = async move {
                        let buf = inner.read(size).await?;

                        Ok(buf)
                    };
                    *read_fut = Some(Mutex::new(Box::pin(fut)));
                }
            }
        }
    }
}
