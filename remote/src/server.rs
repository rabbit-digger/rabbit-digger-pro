use crate::protocol::Protocol;
use rd_interface::{async_trait, Arc, IServer, Net, Result};

pub struct RemoteServer {
    net: Net,
    protocol: Arc<dyn Protocol>,
}

#[async_trait]
impl IServer for RemoteServer {
    async fn start(&self) -> Result<()> {
        todo!()
    }
}

impl RemoteServer {
    pub fn new(protocol: Arc<dyn Protocol>, net: Net) -> RemoteServer {
        RemoteServer { net, protocol }
    }
}
