use std::{io, net::SocketAddr, pin::Pin, task, time::Duration};

use crate::stream::IOStream;
use futures::{ready, FutureExt};
use rd_interface::{async_trait, AsyncRead, AsyncWrite, ITcpStream, ReadBuf, NOT_IMPLEMENTED};
use tokio::time::{sleep, Sleep};

pub(super) struct TrojanTcp {
    stream: Box<dyn IOStream>,
    head: Option<Vec<u8>>,
    is_first: bool,
    sleep: Pin<Box<Sleep>>,
}

impl TrojanTcp {
    pub fn new(stream: Box<dyn IOStream>, head: Vec<u8>) -> Self {
        Self {
            stream,
            head: Some(head),
            is_first: true,
            sleep: Box::pin(sleep(Duration::from_millis(100))),
        }
    }
}

#[async_trait]
impl ITcpStream for TrojanTcp {
    async fn peer_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    async fn local_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    fn poll_read(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> task::Poll<io::Result<()>> {
        loop {
            let Self {
                stream,
                head,
                is_first,
                sleep,
            } = &mut *self;
            let stream = Pin::new(stream);

            if sleep.is_elapsed() {
                *is_first = false;

                let len = match head {
                    Some(head) => {
                        let sent = ready!(stream.poll_write(cx, head))?;
                        head.drain(..sent);
                        head.len()
                    }
                    None => break,
                };
                if len == 0 {
                    *head = None;
                    break;
                }
            }
            if let task::Poll::Pending = sleep.poll_unpin(cx) {
                break;
            }
        }

        Pin::new(&mut self.stream).poll_read(cx, buf)
    }

    fn poll_write(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> task::Poll<io::Result<usize>> {
        loop {
            let Self {
                stream,
                head,
                is_first,
                ..
            } = &mut *self;
            let stream = Pin::new(stream);
            let len = match head {
                Some(head) => {
                    if *is_first {
                        head.extend(buf);
                        *is_first = false;
                    }

                    let sent = ready!(stream.poll_write(cx, head))?;
                    head.drain(..sent);
                    head.len()
                }
                None => break,
            };
            if len == 0 {
                *head = None;
                return task::Poll::Ready(Ok(buf.len()));
            }
        }

        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(&mut self, cx: &mut task::Context<'_>) -> task::Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(&mut self, cx: &mut task::Context<'_>) -> task::Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
