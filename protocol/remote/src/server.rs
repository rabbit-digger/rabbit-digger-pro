use std::sync::atomic::{AtomicU32, Ordering};

use crate::protocol::{Channel, CommandRequest, CommandResponse, Protocol};
use dashmap::DashMap;
use rd_interface::{async_trait, Arc, Context, Error, IServer, Net, Result, TcpStream};
use rd_std::util::connect_tcp;

#[derive(Clone)]
struct Map(Arc<(DashMap<u32, TcpStream>, AtomicU32)>);

impl Map {
    fn new() -> Map {
        Map(Arc::new((DashMap::new(), AtomicU32::new(0))))
    }
    fn insert(&self, tcp: TcpStream) -> u32 {
        let id = self.0 .1.fetch_add(1, Ordering::SeqCst);
        self.0 .0.insert(id, tcp);
        id
    }
    fn get(&self, id: u32) -> Option<TcpStream> {
        self.0 .0.remove(&id).map(|i| i.1)
    }
}

pub struct RemoteServer {
    net: Net,
    protocol: Arc<dyn Protocol>,
}

#[async_trait]
impl IServer for RemoteServer {
    async fn start(&self) -> Result<()> {
        let map = Map::new();

        loop {
            let channel = self.protocol.channel().await?;
            tokio::spawn(process_channel(channel, self.net.clone(), map.clone()));
        }
    }
}

async fn process_channel(mut channel: Channel, net: Net, map: Map) -> Result<()> {
    let req: CommandRequest = channel.recv().await?;

    match req {
        CommandRequest::TcpConnect { address } => {
            let ctx = &mut Context::new();
            let target = net.tcp_connect(ctx, &address).await?;
            connect_tcp(ctx, target, channel.into_inner()).await?;
        }
        CommandRequest::TcpBind { address } => {
            let listener = net.tcp_bind(&mut Context::new(), &address).await?;
            channel
                .send(CommandResponse::BindAddr {
                    addr: listener.local_addr().await?,
                })
                .await?;

            loop {
                let (tcp, addr) = listener.accept().await?;
                let id = map.insert(tcp);
                channel.send(CommandResponse::Accept { id, addr }).await?;
            }
        }
        CommandRequest::TcpAccept { id } => {
            let target = map
                .get(id)
                .ok_or_else(|| Error::Other("ID is not found".into()))?;
            connect_tcp(&mut Context::new(), target, channel.into_inner()).await?;
        }
    }

    Ok(())
}

impl RemoteServer {
    pub fn new(protocol: Arc<dyn Protocol>, net: Net) -> RemoteServer {
        RemoteServer { net, protocol }
    }
}
