use std::io;

use crate::types::{Request, Response};
use futures::{SinkExt, StreamExt};
use rd_interface::TcpStream;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    sync::Mutex,
};
use tokio_serde::formats::Bincode;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

#[derive(Debug, Deserialize, Serialize)]
struct WithDataSize<T>(T, u32);

type FramedConnStream<Item, SinkSink> = tokio_serde::Framed<
    tokio_util::codec::FramedRead<ReadHalf<TcpStream>, LengthDelimitedCodec>,
    WithDataSize<Item>,
    WithDataSize<SinkSink>,
    Bincode<WithDataSize<Item>, WithDataSize<SinkSink>>,
>;
type FramedConnSink<Item, SinkSink> = tokio_serde::Framed<
    tokio_util::codec::FramedWrite<WriteHalf<TcpStream>, LengthDelimitedCodec>,
    WithDataSize<Item>,
    WithDataSize<SinkSink>,
    Bincode<WithDataSize<Item>, WithDataSize<SinkSink>>,
>;

pub struct Connection<Item, SinkSink> {
    stream: Mutex<FramedConnStream<Item, SinkSink>>,
    sink: Mutex<FramedConnSink<Item, SinkSink>>,
}
pub type ClientConnection = Connection<Response, Request>;
pub type ServerConnection = Connection<Request, Response>;

impl<Item, SinkSink> Connection<Item, SinkSink>
where
    Item: Serialize + DeserializeOwned + Unpin,
    SinkSink: Serialize + DeserializeOwned + Unpin,
{
    pub fn new(tcp: TcpStream) -> Connection<Item, SinkSink> {
        let (read, write) = split(tcp);
        let stream = tokio_serde::Framed::new(
            FramedRead::new(read, LengthDelimitedCodec::new()),
            Bincode::default(),
        );
        let sink = tokio_serde::Framed::new(
            FramedWrite::new(write, LengthDelimitedCodec::new()),
            Bincode::default(),
        );

        Connection {
            stream: Mutex::new(stream),
            sink: Mutex::new(sink),
        }
    }
    pub async fn send(&self, req: SinkSink, data: Option<&[u8]>) -> io::Result<()> {
        let mut sink = self.sink.lock().await;

        let data_size = data.map(|d| d.len() as u32).unwrap_or(0);
        sink.send(WithDataSize(req, data_size)).await?;

        if let Some(data) = data {
            sink.get_mut().get_mut().write_all(data).await?;
        }

        Ok(())
    }
    pub async fn next(&self) -> io::Result<(Item, Vec<u8>)> {
        let mut stream = self.stream.lock().await;

        let WithDataSize(resp, data_size) = match stream.next().await {
            Some(r) => r?,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed",
                ))
            }
        };

        let mut data = Vec::new();
        if data_size > 0 {
            data = vec![0u8; data_size as usize];
            stream.get_mut().get_mut().read_exact(&mut data).await?;
        }

        Ok((resp, data))
    }
}
