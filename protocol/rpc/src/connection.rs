use std::{fmt::Debug, io, marker::PhantomData};

use crate::types::{Request, Response};
use rd_interface::TcpStream;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    sync::Mutex,
};

// 1MB
const MAX_ITEM_SIZE: u32 = 1 * 1024 * 1024;
const MAX_DATA_SIZE: u32 = 1 * 1024 * 1024;

pub struct Connection<Item, SinkSink> {
    read: Mutex<ReadHalf<TcpStream>>,
    write: Mutex<WriteHalf<TcpStream>>,
    _mark: PhantomData<(Item, SinkSink)>,
}
pub type ClientConnection = Connection<Response, Request>;
pub type ServerConnection = Connection<Request, Response>;

fn map_err<E>(e: E) -> io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    io::Error::new(io::ErrorKind::Other, e)
}

impl<Item, SinkSink> Connection<Item, SinkSink>
where
    Item: Serialize + DeserializeOwned + Unpin + Debug,
    SinkSink: Serialize + DeserializeOwned + Unpin,
{
    pub fn new(tcp: TcpStream) -> Connection<Item, SinkSink> {
        let (read, write) = split(tcp);

        Connection {
            read: Mutex::new(read),
            write: Mutex::new(write),
            _mark: PhantomData,
        }
    }
    pub async fn send(&self, req: SinkSink, data: Option<&[u8]>) -> io::Result<()> {
        let mut writer = self.write.lock().await;

        let req = serde_json::to_vec(&req).map_err(map_err)?;
        let data_size = data.map(|d| d.len() as u32).unwrap_or(0);
        writer.write_u32(req.len() as u32).await?;
        writer.write_u32(data_size as u32).await?;
        writer.write_all(&req).await?;
        if let Some(data) = data {
            writer.write_all(data).await?;
        }
        writer.flush().await?;

        Ok(())
    }
    pub async fn next(&self) -> io::Result<(Item, Vec<u8>)> {
        let mut reader = self.read.lock().await;

        let item_size = reader.read_u32().await?;
        let data_size = reader.read_u32().await?;

        if item_size > MAX_ITEM_SIZE || data_size > MAX_DATA_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "item size {} or data size {} is too big",
                    item_size, data_size
                ),
            ));
        }

        let mut item_buf = vec![0; item_size as usize];
        reader.read_exact(&mut item_buf).await?;
        let mut data_buf = vec![0; data_size as usize];
        if data_size > 0 {
            reader.read_exact(&mut data_buf).await?;
        }

        let item = serde_json::from_slice(&item_buf).map_err(map_err)?;

        Ok((item, data_buf))
    }
    #[allow(dead_code)]
    pub async fn close(&self) -> io::Result<()> {
        self.write.lock().await.shutdown().await
    }
}
