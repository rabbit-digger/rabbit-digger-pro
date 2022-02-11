use rd_interface::{Address, Arc, Context, Net, Result};
use state::ClientSessionState;
use tokio::sync::oneshot;

mod state;

use crate::{
    connection::ClientConnection,
    types::{Command, Request, Response},
};

#[derive(Clone)]
pub struct ClientSession {
    conn: Arc<ClientConnection>,
    state: Arc<ClientSessionState>,
}

impl ClientSession {
    pub async fn new(net: &Net, endpoint: &Address) -> Result<Self> {
        let tcp = net.tcp_connect(&mut Context::new(), endpoint).await?;

        let t = Self {
            conn: Arc::new(ClientConnection::new(tcp)),
            state: Arc::new(ClientSessionState::new()),
        };

        t.send(Command::Handshake(t.state.session_id()), None)
            .await?
            .wait()
            .await?
            .0
            .to_null()?;

        Ok(t)
    }

    async fn wait_response(&self) -> Result<()> {
        let (resp, data) = self.conn.next().await?;

        self.state.send_response(resp, data);

        Ok(())
    }

    pub async fn send(&self, cmd: Command, data: Option<&[u8]>) -> Result<ResponseGetter> {
        let seq_id = self.state.next_seq_id();
        self.conn.send(Request { cmd, seq_id }, data).await?;

        let rx = self.state.wait_for_response(seq_id);

        let conn = self.clone();
        tokio::spawn(async move { conn.wait_response().await });
        Ok(ResponseGetter { rx })
    }
}

pub struct ResponseGetter {
    rx: oneshot::Receiver<(Response, Vec<u8>)>,
}

impl ResponseGetter {
    pub async fn wait(self) -> Result<(Response, Vec<u8>)> {
        self.rx
            .await
            .map_err(|_| rd_interface::Error::other("channel closed"))
    }
}
