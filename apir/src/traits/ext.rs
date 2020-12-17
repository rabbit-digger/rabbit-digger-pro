use super::runtime::{async_trait, UdpSocket};
use std::{
    io::{ErrorKind, Result},
    net::SocketAddr,
    sync::RwLock,
};

pub struct UdpSocketConnect<U: UdpSocket>(U, RwLock<Option<SocketAddr>>);

#[async_trait]
impl<U> UdpSocket for UdpSocketConnect<U>
where
    U: UdpSocket,
{
    async fn recv_from(&self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        self.0.recv_from(buf).await
    }
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> std::io::Result<usize> {
        self.0.send_to(buf, addr).await
    }
    async fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.0.local_addr().await
    }
}

impl<U: UdpSocket> UdpSocketConnect<U> {
    pub fn new(u: U) -> Self {
        Self(u, RwLock::new(None))
    }
    pub async fn connect(&self, addr: SocketAddr) -> Result<()> {
        *self.1.write().unwrap() = Some(addr);
        Ok(())
    }
    pub async fn send(&self, buf: &[u8]) -> Result<usize> {
        let addr = self.1.read().unwrap();
        let addr = match *addr {
            Some(addr) => addr.clone(),
            None => return Err(ErrorKind::AddrNotAvailable.into()),
        };
        self.0.send_to(buf, addr).await
    }
    pub async fn recv_from(&self, buf: &mut [u8]) -> Result<usize> {
        loop {
            let addr = self.1.read().unwrap();
            let addr = match *addr {
                Some(addr) => addr.clone(),
                None => return Err(ErrorKind::AddrNotAvailable.into()),
            };
            let (size, raddr) = self.0.recv_from(buf).await?;
            if addr == raddr {
                return Ok(size);
            }
        }
    }
    pub fn into_inner(self) -> U {
        self.0
    }
}
