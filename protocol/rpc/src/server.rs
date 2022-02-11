use std::collections::HashMap;

use parking_lot::Mutex;
use rd_interface::{async_trait, Address, Arc, Context, IServer, Net, Result, TcpStream};
use uuid::Uuid;

use crate::{
    connection::ServerConnection,
    types::{Command, Request},
};

const MAX_SESSION_SIZE: usize = 8;

pub struct RpcServer {
    listen: Net,
    net: Net,
    bind: Address,

    sessions: Arc<Mutex<HashMap<Uuid, ServerConnection>>>,
}
impl RpcServer {
    pub fn new(listen: Net, net: Net, bind: Address) -> RpcServer {
        RpcServer {
            listen,
            net,
            bind,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl IServer for RpcServer {
    async fn start(&self) -> Result<()> {
        let listener = self
            .listen
            .tcp_bind(&mut Context::new(), &self.bind)
            .await?;

        loop {
            let (conn, _) = listener.accept().await?;
            tokio::spawn(handle_conn(conn, self.sessions.clone()));
        }
    }
}

async fn handle_conn(
    tcp: TcpStream,
    sessions: Arc<Mutex<HashMap<Uuid, ServerConnection>>>,
) -> Result<()> {
    let conn = ServerConnection::new(tcp);
    let (req, _) = conn.next().await?;

    // TODO: handle session_id
    let session_id = match req.cmd {
        Command::Handshake(session_id) => session_id,
        _ => return Err(rd_interface::Error::other("Invalid handshake")),
    };

    sessions.lock().insert(session_id, conn);

    todo!()
}
