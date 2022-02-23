use crate::session::ClientSession;
use crate::types::Command;

use self::socket::TcpListenerWrapper;

use rd_interface::{async_trait, Address, Context, INet, IntoDyn, Net, Result, TcpStream};

use socket::{TcpWrapper, UdpWrapper};
use tokio::sync::OnceCell;

mod socket;

pub struct RpcNet {
    net: Net,
    endpoint: Address,

    sess: OnceCell<Result<ClientSession>>,
}

impl RpcNet {
    pub fn new(net: Net, endpoint: Address) -> Self {
        RpcNet {
            net,
            endpoint,
            sess: OnceCell::new(),
        }
    }
    async fn get_sess(&self) -> Result<&ClientSession> {
        self.sess
            .get_or_init(|| ClientSession::new(&self.net, &self.endpoint))
            .await
            .as_ref()
            .map_err(|e| rd_interface::Error::other(e.to_string()))
    }
}

#[async_trait]
impl INet for RpcNet {
    async fn tcp_connect(&self, ctx: &mut Context, addr: &Address) -> Result<TcpStream> {
        let conn = self.get_sess().await?.clone();
        let (resp, _) = conn
            .send(Command::TcpConnect(ctx.to_value(), addr.clone()), None)
            .await?
            .wait()
            .await?;

        let tcp = TcpWrapper::new(conn, resp.into_object()?);

        Ok(tcp.into_dyn())
    }

    async fn tcp_bind(
        &self,
        ctx: &mut Context,
        addr: &Address,
    ) -> Result<rd_interface::TcpListener> {
        let conn = self.get_sess().await?.clone();
        let (resp, _) = conn
            .send(Command::TcpBind(ctx.to_value(), addr.clone()), None)
            .await?
            .wait()
            .await?;

        let listener = TcpListenerWrapper::new(conn, resp.into_object()?);

        Ok(listener.into_dyn())
    }

    async fn udp_bind(&self, ctx: &mut Context, addr: &Address) -> Result<rd_interface::UdpSocket> {
        let conn = self.get_sess().await?.clone();
        let (resp, _) = conn
            .send(Command::UdpBind(ctx.to_value(), addr.clone()), None)
            .await?
            .wait()
            .await?;

        let udp = UdpWrapper::new(conn, resp.into_object()?);

        Ok(udp.into_dyn())
    }

    async fn lookup_host(&self, addr: &Address) -> Result<Vec<std::net::SocketAddr>> {
        let conn = self.get_sess().await?.clone();
        let getter = conn.send(Command::LookupHost(addr.clone()), None).await?;

        let (resp, _) = getter.wait().await?;

        resp.into_value()
    }
}
