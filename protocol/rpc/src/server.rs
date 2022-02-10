use rd_interface::{async_trait, Address, IServer, Net, Result};

pub struct RpcServer {
    listen: Net,
    net: Net,
    bind: Address,
}
impl RpcServer {
    pub fn new(listen: Net, net: Net, bind: Address) -> RpcServer {
        RpcServer { listen, net, bind }
    }
}

#[async_trait]
impl IServer for RpcServer {
    async fn start(&self) -> Result<()> {
        todo!()
    }
}
