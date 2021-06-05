use std::net::SocketAddr;

use rd_interface::{
    async_trait,
    schemars::{self, JsonSchema},
    Address, Arc, Config, Context, Error, IntoAddress, Net, Result, TcpListener, TcpStream,
};
use serde_derive::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;

#[derive(Deserialize, JsonSchema, Config)]
pub enum Connection {
    Active { remote: String },
    Passive { bind: String },
}

#[derive(Deserialize, JsonSchema, Config)]
pub struct Config {
    conn: Connection,
    udp_in_tcp: bool,
    token: String,
}

pub struct ActiveProtocol {
    net: Net,
    remote: String,
    token: String,
}

pub struct PassiveProtocol {
    net: Net,
    bind: String,
    listener: RwLock<Option<TcpListener>>,
    token: String,
}

#[async_trait]
pub trait Protocol: Send + Sync + 'static {
    async fn channel(&self) -> Result<Channel>;
}

#[async_trait]
impl Protocol for ActiveProtocol {
    async fn channel(&self) -> Result<Channel> {
        let mut conn = self
            .net
            .tcp_connect(&mut Context::new(), self.remote.into_address()?)
            .await?;

        handshake(&mut conn, &self.token).await?;

        Ok(Channel { tcp: conn })
    }
}

#[async_trait]
impl Protocol for PassiveProtocol {
    async fn channel(&self) -> Result<Channel> {
        let (mut conn, _addr) = self.accept().await?;

        handshake(&mut conn, &self.token).await?;

        Ok(Channel { tcp: conn })
    }
}

impl PassiveProtocol {
    async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        if let Some(f) = self.listener.read().await.as_ref() {
            return f.accept().await;
        }
        let mut listener = self.listener.write().await;
        let new_one = match listener.as_ref() {
            Some(f) => return f.accept().await,
            None => {
                let listener = self
                    .net
                    .tcp_bind(&mut Context::new(), self.bind.into_address()?)
                    .await?;
                listener
            }
        };
        let r = new_one.accept().await;
        *listener = Some(new_one);
        return r;
    }
}

pub fn get_protocol(net: Net, config: Config) -> Result<Arc<dyn Protocol>> {
    let token = config.token;
    Ok(match config.conn {
        Connection::Active { remote } => {
            Arc::new(ActiveProtocol { net, remote, token }) as Arc<dyn Protocol>
        }
        Connection::Passive { bind } => Arc::new(PassiveProtocol {
            net,
            bind,
            token,
            listener: RwLock::new(None),
        }) as Arc<dyn Protocol>,
    })
}

async fn handshake(channel: &mut TcpStream, token: &str) -> Result<()> {
    let mut buf = vec![0u8; token.len()];
    channel.read_exact(&mut buf).await?;

    if buf != token.as_bytes() {
        return Err(Error::Other("Handshake failed".into()));
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CommandRequest {
    TcpConnect { address: Address },
    TcpBind { address: Address },
    TcpAccept { id: u64 },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CommandResponse {
    Accept { id: u64, addr: SocketAddr },
    BindAddr { addr: SocketAddr },
}

pub struct Channel {
    tcp: TcpStream,
}

impl Channel {
    pub async fn send(&mut self, cmd: CommandRequest) -> Result<()> {
        let channel = &mut self.tcp;
        let cmd = bincode::serialize(&cmd).map_err(|e| Error::Other(e))?;
        channel.write_u16(cmd.len() as u16).await?;
        channel.write_all(&cmd).await?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<CommandResponse> {
        let channel = &mut self.tcp;
        let len = channel.read_u16().await?;
        let mut buf = vec![0u8; len as usize];
        channel.read_exact(&mut buf).await?;
        let result = bincode::deserialize(&buf).map_err(|e| Error::Other(e))?;
        Ok(result)
    }

    pub fn into_inner(self) -> TcpStream {
        self.tcp
    }
}
