use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
};

use crate::types::{Command, Object};

use crate::connection::Connection;
use futures::{ready, Future, FutureExt, Sink, Stream};
use parking_lot::Mutex;
use rd_interface::{
    async_trait, impl_async_read_write, Address, Bytes, BytesMut, ITcpListener, ITcpStream,
    IUdpSocket, IntoDyn, Result,
};
use rd_std::util::async_fn_io::{AsyncFnIO, AsyncFnRead, AsyncFnWrite};

pub struct TcpListenerWrapper {
    conn: Connection,
    obj: Object,
}

impl TcpListenerWrapper {
    pub fn new(conn: Connection, obj: Object) -> Self {
        Self { conn, obj }
    }
}

#[async_trait]
impl ITcpListener for TcpListenerWrapper {
    async fn accept(&self) -> Result<(rd_interface::TcpStream, SocketAddr)> {
        let (resp, _) = self
            .conn
            .send(Command::Accept(self.obj), None)
            .await?
            .wait()
            .await?;
        let (obj, addr) = resp.to_object_value()?;

        Ok((TcpWrapper::new(self.conn.clone(), obj).into_dyn(), addr))
    }

    async fn local_addr(&self) -> Result<SocketAddr> {
        let (resp, _) = self
            .conn
            .send(Command::LocalAddr(self.obj), None)
            .await?
            .wait()
            .await?;
        resp.to_value()
    }
}

type BoxFuture<T> = Mutex<Pin<Box<dyn Future<Output = T> + Send + 'static>>>;
pub struct UdpWrapper {
    conn: Connection,
    obj: Object,

    next_fut: Option<BoxFuture<io::Result<(BytesMut, SocketAddr)>>>,
    send_fut: Option<BoxFuture<io::Result<()>>>,
}

impl UdpWrapper {
    pub fn new(conn: Connection, obj: Object) -> Self {
        UdpWrapper {
            conn,
            obj,

            next_fut: None,
            send_fut: None,
        }
    }
}

impl Stream for UdpWrapper {
    type Item = io::Result<(BytesMut, SocketAddr)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let UdpWrapper {
            next_fut,
            conn,
            obj,
            ..
        } = &mut *self;
        let obj = *obj;

        loop {
            match next_fut {
                Some(fut) => {
                    let i = ready!(fut.lock().poll_unpin(cx));

                    *next_fut = None;
                    return Poll::Ready(Some(i));
                }
                None => {
                    let conn = conn.clone();
                    let fut = async move {
                        let (resp, data) = conn
                            .send(Command::RecvFrom(obj, 4096), None)
                            .await?
                            .wait()
                            .await?;

                        Ok((BytesMut::from(&data[..]), resp.to_value()?))
                    };
                    *next_fut = Some(Mutex::new(Box::pin(fut)));
                }
            }
        }
    }
}

impl Sink<(Bytes, Address)> for UdpWrapper {
    type Error = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let send_fut = &mut self.send_fut;
        match send_fut {
            Some(fut) => {
                ready!(fut.lock().poll_unpin(cx))?;

                *send_fut = None;
                Poll::Ready(Ok(()))
            }
            None => Poll::Ready(Ok(())),
        }
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        (bytes, addr): (Bytes, Address),
    ) -> Result<(), Self::Error> {
        let UdpWrapper {
            send_fut,
            conn,
            obj,
            ..
        } = &mut *self;
        match send_fut {
            Some(_) => return Err(io::Error::new(io::ErrorKind::Other, "already sending")),
            None => {
                let conn = conn.clone();
                let obj = *obj;
                let fut = async move {
                    let (resp, _) = conn
                        .send(Command::SendTo(obj, addr), Some(&bytes))
                        .await?
                        .wait()
                        .await?;
                    resp.to_null()?;
                    Ok(())
                };
                *send_fut = Some(Mutex::new(Box::pin(fut)));
            }
        }

        Ok(())
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.poll_ready(cx)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}

#[async_trait]
impl IUdpSocket for UdpWrapper {
    async fn local_addr(&self) -> Result<SocketAddr> {
        let (resp, _) = self
            .conn
            .send(Command::LocalAddr(self.obj), None)
            .await?
            .wait()
            .await?;
        resp.to_value()
    }
}

#[derive(Clone)]
pub struct TcpAsyncFn {
    conn: Connection,
    obj: Object,
}

impl TcpWrapper {
    pub fn new(conn: Connection, obj: Object) -> TcpWrapper {
        TcpWrapper(AsyncFnIO::new(TcpAsyncFn { conn, obj }))
    }
}

pub struct TcpWrapper(AsyncFnIO<TcpAsyncFn>);

#[async_trait]
impl AsyncFnRead for TcpAsyncFn {
    async fn read(&mut self, buf_size: usize) -> io::Result<Vec<u8>> {
        let getter = self
            .conn
            .send(Command::Read(self.obj, buf_size as u32), None)
            .await?;

        let (resp, data) = getter.wait().await?;
        resp.to_null()?;
        Ok(data)
    }
}

#[async_trait]
impl AsyncFnWrite for TcpAsyncFn {
    async fn write(&mut self, buf: Vec<u8>) -> io::Result<usize> {
        let getter = self.conn.send(Command::Write(self.obj), Some(&buf)).await?;

        let (resp, _) = getter.wait().await?;
        let size = resp.to_value::<u32>()?;

        Ok(size as usize)
    }

    async fn flush(&mut self) -> io::Result<()> {
        let getter = self.conn.send(Command::Flush(self.obj), None).await?;

        getter.wait().await?;

        Ok(())
    }

    async fn shutdown(&mut self) -> io::Result<()> {
        let getter = self.conn.send(Command::Shutdown(self.obj), None).await?;

        getter.wait().await?;

        Ok(())
    }
}

impl_async_read_write!(TcpWrapper, 0);

#[async_trait]
impl ITcpStream for TcpWrapper {
    async fn peer_addr(&self) -> Result<std::net::SocketAddr> {
        let this = self.0.get_ref();
        let (resp, _) = this
            .conn
            .send(Command::PeerAddr(this.obj), None)
            .await?
            .wait()
            .await?;
        resp.to_value()
    }

    async fn local_addr(&self) -> Result<std::net::SocketAddr> {
        let this = self.0.get_ref();
        let (resp, _) = this
            .conn
            .send(Command::LocalAddr(this.obj), None)
            .await?
            .wait()
            .await?;
        resp.to_value()
    }
}
