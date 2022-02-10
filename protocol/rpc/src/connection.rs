use std::collections::HashMap;

use futures::{SinkExt, StreamExt};
use parking_lot::Mutex as SyncMutex;
use rd_interface::{Address, Arc, Context, Net, Result, TcpStream};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    sync::{oneshot, Mutex},
};
use tokio_serde::formats::Bincode;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use uuid::Uuid;

use crate::types::{Command, Request, Response};

type FramedConnStream = tokio_serde::Framed<
    tokio_util::codec::FramedRead<ReadHalf<TcpStream>, LengthDelimitedCodec>,
    Response,
    Request,
    Bincode<Response, Request>,
>;
type FramedConnSink = tokio_serde::Framed<
    tokio_util::codec::FramedWrite<WriteHalf<TcpStream>, LengthDelimitedCodec>,
    Response,
    Request,
    Bincode<Response, Request>,
>;

struct ConnState {
    stream: Mutex<FramedConnStream>,
    sink: Mutex<FramedConnSink>,

    seq_id: SyncMutex<u32>,
    wait_map: SyncMutex<HashMap<u32, oneshot::Sender<(Response, Vec<u8>)>>>,
}

#[derive(Clone)]
pub struct Connection {
    session_id: Uuid,

    state: Arc<ConnState>,
}

impl Connection {
    pub async fn new(net: &Net, endpoint: &Address) -> Result<Self> {
        let tcp = net.tcp_connect(&mut Context::new(), endpoint).await?;
        let (read, write) = split(tcp);

        let stream = tokio_serde::Framed::new(
            FramedRead::new(read, LengthDelimitedCodec::new()),
            Bincode::default(),
        );
        let sink = tokio_serde::Framed::new(
            FramedWrite::new(write, LengthDelimitedCodec::new()),
            Bincode::default(),
        );
        let t = Self {
            session_id: Uuid::new_v4(),
            state: Arc::new(ConnState {
                stream: Mutex::new(stream),
                sink: Mutex::new(sink),
                seq_id: SyncMutex::new(0),
                wait_map: SyncMutex::new(HashMap::new()),
            }),
        };

        t.send(Command::Handshake(t.session_id), None).await?;

        Ok(t)
    }

    async fn wait_response(&self) -> Result<()> {
        let state = &self.state;
        let mut stream = state.stream.lock().await;
        let resp: Response = match stream.next().await {
            Some(item) => item?,
            None => return Err(rd_interface::Error::other("connection closed")),
        };

        let mut data = Vec::new();
        if resp.data_size > 0 {
            data = vec![0u8; resp.data_size as usize];
            stream.get_mut().get_mut().read_exact(&mut data).await?;
        }

        if let Some(tx) = state.wait_map.lock().remove(&resp.seq_id) {
            let _ = tx.send((resp, data));
        }

        Ok(())
    }

    pub async fn send(&self, cmd: Command, data: Option<&[u8]>) -> Result<ResponseGetter> {
        let state = &self.state;

        let mut seq_id = state.seq_id.lock();
        *seq_id += 1;
        let mut sink = state.sink.lock().await;

        sink.send(Request {
            cmd,
            seq_id: *seq_id,
            data_size: data.map(|d| d.len() as u32).unwrap_or(0),
        })
        .await?;

        if let Some(data) = data {
            sink.get_mut().get_mut().write_all(data).await?;
        }

        let (tx, rx) = oneshot::channel();
        state.wait_map.lock().insert(*seq_id, tx);
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
