use std::{
    io::{Cursor, Write},
    mem::take,
    net::SocketAddr,
    sync::RwLock,
};

use super::ra2sa;
use crate::stream::IOStream;
use rd_interface::{async_trait, Address as RDAddress, IUdpSocket, Result, NOT_IMPLEMENTED};
use socks5_protocol::{sync::FromIO, Address as S5Addr};
use tokio::{
    io::{self, split, AsyncReadExt, ReadHalf, WriteHalf},
    sync::Mutex,
};

pub(super) struct TrojanUdp {
    read: Mutex<ReadHalf<Box<dyn IOStream>>>,
    write: Mutex<WriteHalf<Box<dyn IOStream>>>,
    head: RwLock<Vec<u8>>,
}

impl TrojanUdp {
    pub fn new(stream: Box<dyn IOStream>, head: Vec<u8>) -> Self {
        let (read, write) = split(stream);
        Self {
            read: Mutex::new(read),
            write: Mutex::new(write),
            head: RwLock::new(head),
        }
    }
}

#[async_trait]
impl IUdpSocket for TrojanUdp {
    async fn local_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }

    async fn recv_from(&self, recv_buf: &mut [u8]) -> rd_interface::Result<(usize, SocketAddr)> {
        let mut read = self.read.lock().await;
        let address = S5Addr::read(&mut *read)
            .await
            .map_err(|e| e.to_io_err())?
            .to_socket_addr()
            .map_err(|e| e.to_io_err())?;
        let length = read.read_u16().await? as usize;
        let _crlf = read.read_u16().await?;

        let to_read = length.min(recv_buf.len());
        let rest = length - to_read;
        read.read_exact(&mut recv_buf[..to_read]).await?;
        if rest > 0 {
            read.read_exact(&mut vec![0u8; rest]).await?;
        }

        Ok((to_read, address))
    }

    async fn send_to(&self, payload: &[u8], target: RDAddress) -> Result<usize> {
        if payload.len() > 65535 {
            return Err(io::Error::from(io::ErrorKind::InvalidData).into());
        }
        let addr = ra2sa(target);
        let buf = if self.head.read().unwrap().len() > 0 {
            let mut head = self.head.write().unwrap();
            if head.len() > 0 {
                take(&mut *head)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let pos = buf.len() as u64;
        let mut writer = Cursor::new(buf);
        writer.set_position(pos);

        addr.write_to(&mut writer).map_err(|e| e.to_io_err())?;
        writer.write_all(&u16::to_be_bytes(payload.len() as u16))?;
        writer.write_all(b"\r\n")?;
        writer.write_all(payload)?;
        let buf = writer.into_inner();

        io::AsyncWriteExt::write_all(&mut *self.write.lock().await, &buf).await?;

        Ok(payload.len())
    }
}
