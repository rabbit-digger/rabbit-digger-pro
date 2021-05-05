use std::{future::Future, io, net::SocketAddr, sync::Arc};

use futures_executor::ThreadPool;
use futures_util::{
    future::try_join,
    task::{SpawnError, SpawnExt},
};

use crate::{async_trait, Address, Result, UdpSocket};

#[derive(Clone)]
pub struct ConnectionPool {
    pool: ThreadPool,
}

impl ConnectionPool {
    pub fn new() -> io::Result<ConnectionPool> {
        let pool = ThreadPool::new()?;
        Ok(ConnectionPool { pool })
    }
    pub fn spawn<Fut>(&self, future: Fut) -> Result<(), SpawnError>
    where
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.pool.spawn(future)
    }
    pub async fn connect_udp(&self, udp_channel: UdpChannel, udp: UdpSocket) -> crate::Result<()> {
        let in_side = async {
            let mut buf = [0u8; crate::constant::UDP_BUFFER_SIZE];
            while let Ok((size, addr)) = udp_channel.recv_send_to(&mut buf).await {
                let buf = &buf[..size];
                udp.send_to(buf, addr).await?;
            }
            crate::Result::<()>::Ok(())
        };
        let out_side = async {
            let mut buf = [0u8; crate::constant::UDP_BUFFER_SIZE];
            while let Ok((size, addr)) = udp.recv_from(&mut buf).await {
                let buf = &buf[..size];
                udp_channel.send_recv_from(buf, addr).await?;
            }
            crate::Result::<()>::Ok(())
        };
        try_join(in_side, out_side).await?;
        Ok(())
    }
}

#[async_trait]
pub trait IUdpChannel: Send + Sync {
    async fn recv_send_to(&self, data: &mut [u8]) -> Result<(usize, Address)>;
    async fn send_recv_from(&self, data: &[u8], addr: SocketAddr) -> Result<usize>;
}
type UdpChannel = Arc<dyn IUdpChannel>;

impl<T: IUdpChannel> crate::IntoDyn<UdpChannel> for T {
    fn into_dyn(self) -> UdpChannel
    where
        Self: Sized + 'static,
    {
        Arc::new(self)
    }
}
