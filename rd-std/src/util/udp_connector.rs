use std::{
    io,
    mem::replace,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
    time::Duration,
};

use futures::{ready, Future, FutureExt, Sink, SinkExt, Stream, StreamExt};
use parking_lot::Mutex;
use rd_interface::{async_trait, Bytes, BytesMut, IUdpSocket, Result, UdpSocket};
use tokio::{sync::Semaphore, time::timeout};
use tokio_util::sync::PollSemaphore;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
type Connector = Box<dyn FnOnce(&(Bytes, SocketAddr)) -> BoxFuture<Result<UdpSocket>> + Send>;

enum State {
    Idle {
        connector: Mutex<Option<Connector>>,
    },
    Binding {
        fut: Mutex<BoxFuture<Result<UdpSocket>>>,
    },
    Binded(UdpSocket),
    // Dummy state for replace
    Dummy,
}

/// A UdpConnector is a UdpSocket that lazily binds when first packet is about to send.
pub struct UdpConnector {
    semaphore: PollSemaphore,
    state: State,
}

impl UdpConnector {
    pub fn new(connector: Connector) -> Self {
        UdpConnector {
            semaphore: PollSemaphore::new(Semaphore::new(0).into()),
            state: State::Idle {
                connector: Mutex::new(Some(connector)),
            },
        }
    }
}

impl Stream for UdpConnector {
    type Item = std::io::Result<(BytesMut, SocketAddr)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match &mut self.state {
                State::Binded(udp) => return udp.poll_next_unpin(cx),
                State::Idle { .. } | State::Binding { .. } => {
                    ready!(self.semaphore.poll_acquire(cx));
                }
                State::Dummy => unreachable!(),
            }
        }
    }
}

async fn send_first_packet(connector: Connector, item: (Bytes, SocketAddr)) -> Result<UdpSocket> {
    let mut udp = timeout(Duration::from_secs(5), connector(&item)).await??;

    udp.send(item).await?;

    Ok(udp)
}

impl Sink<(Bytes, SocketAddr)> for UdpConnector {
    type Error = std::io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        loop {
            match &mut self.state {
                State::Binded(udp) => return udp.poll_ready_unpin(cx),
                State::Idle { .. } => return Poll::Ready(Ok(())),
                State::Binding { fut } => {
                    let udp = ready!(fut.lock().poll_unpin(cx));
                    self.state = State::Binded(udp?);
                    self.semaphore.add_permits(1);
                }
                State::Dummy => unreachable!(),
            }
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: (Bytes, SocketAddr)) -> Result<(), Self::Error> {
        let result: Result<(), Self::Error>;

        let old_state = replace(&mut self.state, State::Dummy);

        self.state = match old_state {
            State::Idle { connector } => {
                result = Ok(());
                State::Binding {
                    fut: Mutex::new(Box::pin(send_first_packet(
                        connector
                            .lock()
                            .take()
                            .expect("connector shouldn't be None"),
                        item,
                    ))),
                }
            }
            State::Binded(mut udp) => {
                result = udp.start_send_unpin(item);
                State::Binded(udp)
            }
            State::Binding { .. } => {
                result = Err(io::Error::new(
                    io::ErrorKind::Other,
                    "start_send called twice",
                ));
                old_state
            }
            State::Dummy => unreachable!(),
        };

        result
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        loop {
            match &mut self.state {
                State::Binded(udp) => return udp.poll_flush_unpin(cx),
                State::Idle { .. } => return Poll::Ready(Ok(())),
                State::Binding { fut } => {
                    let udp = ready!(fut.lock().poll_unpin(cx));
                    self.state = State::Binded(udp?);
                    self.semaphore.add_permits(1);
                }
                State::Dummy => unreachable!(),
            }
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(self.poll_flush_unpin(cx))?;
        match &mut self.state {
            State::Binded(udp) => udp.poll_close_unpin(cx),
            State::Idle { .. } | State::Binding { .. } => Poll::Ready(Ok(())),
            State::Dummy => unreachable!(),
        }
    }
}

#[async_trait]
impl IUdpSocket for UdpConnector {
    async fn local_addr(&self) -> Result<SocketAddr> {
        match &self.state {
            State::Binded(udp) => udp.local_addr().await,
            State::Idle { .. } | State::Binding { .. } => Err(rd_interface::Error::NotFound(
                "UdpConnector is not connected".to_string(),
            )),
            State::Dummy => unreachable!(),
        }
    }
}
