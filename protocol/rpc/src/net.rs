use std::io;

use crate::types::{Command, Object};

use super::connection::Connection;
use rd_interface::{
    async_trait, impl_async_read_write, Address, Context, INet, ITcpStream, IntoDyn, Net, Result,
    TcpStream,
};
use rd_std::util::async_fn_io::{AsyncFnIO, AsyncFnRead, AsyncFnWrite};
use tokio::sync::OnceCell;

pub struct RpcNet {
    net: Net,
    endpoint: Address,

    conn: OnceCell<Result<Connection>>,
}

impl RpcNet {
    pub fn new(net: Net, endpoint: Address) -> Self {
        RpcNet {
            net,
            endpoint,
            conn: OnceCell::new(),
        }
    }
    async fn get_conn(&self) -> Result<&Connection> {
        self.conn
            .get_or_init(|| Connection::new(&self.net, &self.endpoint))
            .await
            .as_ref()
            .map_err(|e| rd_interface::Error::other(e.to_string()))
    }
}

#[async_trait]
impl INet for RpcNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: &Address) -> Result<TcpStream> {
        let conn = self.get_conn().await?.clone();
        let (resp, _) = conn
            .send(Command::TcpConnect(ctx.to_value(), addr.clone()), None)
            .await?
            .wait()
            .await?;

        let tcp = TcpWrapper::new(conn, resp.to_object()?);

        Ok(tcp.into_dyn())
    }

    async fn tcp_bind(
        &self,
        _ctx: &mut Context,
        _addr: &Address,
    ) -> Result<rd_interface::TcpListener> {
        Err(rd_interface::NOT_IMPLEMENTED)
    }

    async fn udp_bind(
        &self,
        _ctx: &mut Context,
        _addr: &Address,
    ) -> Result<rd_interface::UdpSocket> {
        Err(rd_interface::NOT_IMPLEMENTED)
    }

    async fn lookup_host(&self, _addr: &Address) -> Result<Vec<std::net::SocketAddr>> {
        Err(rd_interface::NOT_IMPLEMENTED)
    }
}

#[derive(Clone)]
pub struct TcpAsyncFn {
    conn: Connection,
    obj: Object,
}

impl TcpWrapper {
    fn new(conn: Connection, obj: Object) -> TcpWrapper {
        TcpWrapper(AsyncFnIO::new(TcpAsyncFn { conn, obj }))
    }
}

struct TcpWrapper(AsyncFnIO<TcpAsyncFn>);

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
