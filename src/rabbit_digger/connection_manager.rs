use std::{
    collections::HashMap,
    io,
    sync::{atomic::Ordering, Arc},
    task::{Context, Poll},
    time::{Duration, SystemTime},
};

use super::event::{Event, EventType};
use atomic_shim::AtomicU64;
use dashmap::DashMap;
use futures::{FutureExt, StreamExt};
use parking_lot::Mutex;
use rd_interface::{Address, Value};
use serde::{Serialize, Serializer};
use tokio::{
    sync::{broadcast, mpsc, oneshot},
    task::{unconstrained, JoinHandle},
    time::{interval, sleep},
};
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

fn ts(time: &SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn serialize_atomicu64<S>(a: &AtomicU64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(a.load(Ordering::Relaxed))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, Serialize)]
pub struct ConnectionInfo {
    protocol: Protocol,
    addr: Address,
    start_time: u64,
    ctx: Value,
    #[serde(serialize_with = "serialize_atomicu64")]
    upload: AtomicU64,
    #[serde(serialize_with = "serialize_atomicu64")]
    download: AtomicU64,
    #[serde(skip)]
    stop_sender: Mutex<Option<oneshot::Sender<()>>>,
}

#[derive(Debug, Serialize)]
pub struct ConnectionState {
    connections: DashMap<Uuid, ConnectionInfo>,
    #[serde(serialize_with = "serialize_atomicu64")]
    total_upload: AtomicU64,
    #[serde(serialize_with = "serialize_atomicu64")]
    total_download: AtomicU64,
}

impl ConnectionState {
    fn new() -> Self {
        ConnectionState {
            connections: DashMap::new(),
            total_upload: AtomicU64::new(0),
            total_download: AtomicU64::new(0),
        }
    }
    fn input_event(&self, event: Event) {
        let Event { uuid, events, time } = event;

        for event in events {
            match event {
                EventType::NewTcp(addr, ctx) => {
                    self.connections.insert(
                        uuid,
                        ConnectionInfo {
                            protocol: Protocol::Tcp,
                            addr: addr.clone(),
                            ctx: ctx.clone(),
                            start_time: ts(&time),
                            upload: AtomicU64::new(0),
                            download: AtomicU64::new(0),
                            stop_sender: Mutex::new(None),
                        },
                    );
                }
                EventType::NewUdp(addr, ctx) => {
                    self.connections.insert(
                        uuid,
                        ConnectionInfo {
                            protocol: Protocol::Udp,
                            addr: addr.clone(),
                            ctx: ctx.clone(),
                            start_time: ts(&time),
                            upload: AtomicU64::new(0),
                            download: AtomicU64::new(0),
                            stop_sender: Mutex::new(None),
                        },
                    );
                }
                EventType::SetStopper(sender) => {
                    if let Some(conn) = self.connections.get(&uuid) {
                        let mut stop_sender = conn.stop_sender.lock();
                        *stop_sender = Some(sender);
                    }
                }
                EventType::Read(download) => {
                    if let Some(conn) = self.connections.get(&uuid) {
                        conn.download.fetch_add(download, Ordering::Relaxed);
                        self.total_download.fetch_add(download, Ordering::Relaxed);
                    }
                }
                EventType::Write(upload) => {
                    if let Some(conn) = self.connections.get(&uuid) {
                        conn.upload.fetch_add(upload, Ordering::Relaxed);
                        self.total_upload.fetch_add(upload, Ordering::Relaxed);
                    }
                }
                EventType::RecvFrom(_, download) => {
                    if let Some(conn) = self.connections.get(&uuid) {
                        conn.download.fetch_add(download, Ordering::Relaxed);
                        self.total_download.fetch_add(download, Ordering::Relaxed);
                    }
                }
                EventType::SendTo(_, upload) => {
                    if let Some(conn) = self.connections.get(&uuid) {
                        conn.upload.fetch_add(upload, Ordering::Relaxed);
                        self.total_upload.fetch_add(upload, Ordering::Relaxed);
                    }
                }
                EventType::CloseConnection => {
                    self.connections.remove(&uuid);
                }
            };
        }
    }
    fn input_events<'a>(&self, events: impl Iterator<Item = Event>) {
        for event in events {
            self.input_event(event);
        }
    }
}

struct ManagerInner {
    state: ConnectionState,
    heartbeat_interval: broadcast::Sender<()>,
    sender: mpsc::UnboundedSender<Event>,
    heartbeat_handle: JoinHandle<()>,
}

impl ManagerInner {
    fn new(sender: mpsc::UnboundedSender<Event>) -> Self {
        let (heartbeat_interval, _) = broadcast::channel(1);
        let tx = heartbeat_interval.clone();

        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));
            loop {
                let _ = tx.send(());
                interval.tick().await;
            }
        });

        Self {
            state: ConnectionState::new(),
            heartbeat_interval,
            sender,
            heartbeat_handle,
        }
    }
}

#[derive(Clone)]
pub struct ConnectionManager {
    inner: Arc<ManagerInner>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        let (sender, rx) = mpsc::unbounded_channel();
        let inner = Arc::new(ManagerInner::new(sender));

        tokio::spawn(unconstrained(Self::recv_event(rx, inner.clone())));

        Self { inner }
    }
    #[cfg(test)]
    pub fn new_for_test() -> (Self, mpsc::UnboundedReceiver<Event>) {
        let (sender, rx) = mpsc::unbounded_channel();
        let inner = Arc::new(ManagerInner::new(sender));

        (Self { inner }, rx)
    }
    pub fn stop(&self) {
        self.inner.heartbeat_handle.abort()
    }
    pub fn borrow_state<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&ConnectionState) -> R,
    {
        let conn = &self.inner.state;
        f(conn)
    }
    pub fn stop_connection(&self, uuid: Uuid) -> bool {
        self.inner
            .state
            .connections
            .get(&uuid)
            .and_then(|conn| conn.stop_sender.lock().take())
            .map(|sender| sender.send(()).is_ok())
            .unwrap_or_default()
    }
    async fn recv_event(mut rx: mpsc::UnboundedReceiver<Event>, inner: Arc<ManagerInner>) {
        loop {
            let e = match rx.try_recv() {
                Ok(e) => e,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
                Err(mpsc::error::TryRecvError::Empty) => {
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            let mut events = Vec::with_capacity(32);
            events.push(e);
            while let Ok(e) = rx.try_recv() {
                events.push(e);
            }
            inner.state.input_events(events.into_iter());
        }
        tracing::warn!("recv_event task exited");
    }
    pub fn new_connection<T: ConnType>(
        &self,
        addr: Address,
        ctx: &rd_interface::Context,
    ) -> Connection<T> {
        Connection::<T>::new(
            addr,
            ctx,
            self.inner.heartbeat_interval.subscribe(),
            self.inner.sender.clone(),
        )
    }
}

pub trait ConnType: Default {
    fn event_type(addr: Address, ctx: Value) -> EventType;
    fn get_events(&mut self) -> Vec<EventType>;
}

#[derive(Default)]
pub struct Tcp {
    read: u64,
    write: u64,
}
impl ConnType for Tcp {
    fn event_type(addr: Address, ctx: Value) -> EventType {
        EventType::NewTcp(addr, ctx)
    }

    fn get_events(&mut self) -> Vec<EventType> {
        let mut ret = Vec::with_capacity(2);
        if self.read > 0 {
            ret.push(EventType::Read(self.read));
            self.read = 0;
        }
        if self.write > 0 {
            ret.push(EventType::Write(self.write));
            self.write = 0;
        }
        ret
    }
}
#[derive(Default)]
pub struct Udp {
    recv_from: HashMap<Address, u64>,
    send_to: HashMap<Address, u64>,
}
impl ConnType for Udp {
    fn event_type(addr: Address, ctx: Value) -> EventType {
        EventType::NewUdp(addr, ctx)
    }

    fn get_events(&mut self) -> Vec<EventType> {
        let mut ret = Vec::with_capacity(self.recv_from.len() + self.send_to.len());
        for (addr, download) in self.recv_from.drain() {
            ret.push(EventType::RecvFrom(addr, download));
        }
        for (addr, upload) in self.send_to.drain() {
            ret.push(EventType::SendTo(addr, upload));
        }
        ret
    }
}

#[derive(Debug)]
pub struct Connection<T: ConnType> {
    state: T,
    uuid: Uuid,

    heartbeat_interval: BroadcastStream<()>,
    sender: mpsc::UnboundedSender<Event>,
    stopped: oneshot::Receiver<()>,
}

impl<T> Connection<T>
where
    T: ConnType,
{
    fn new(
        addr: Address,
        ctx: &rd_interface::Context,
        heartbeat_interval: broadcast::Receiver<()>,
        sender: mpsc::UnboundedSender<Event>,
    ) -> Self {
        let (stopper, stopped) = oneshot::channel();
        let uuid = Uuid::new_v4();
        let this = Connection {
            state: T::default(),
            uuid,
            heartbeat_interval: BroadcastStream::new(heartbeat_interval),
            sender,
            stopped,
        };
        this.send(vec![
            T::event_type(addr, ctx.to_value()),
            EventType::SetStopper(stopper),
        ]);
        this
    }
    pub fn poll(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        if let Poll::Ready(_) = self.heartbeat_interval.poll_next_unpin(cx) {
            self.state = T::default();
        }
        if let Poll::Ready(_) = self.stopped.poll_unpin(cx) {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Aborted by user",
            ));
        }
        Ok(())
    }
    fn send(&self, events: Vec<EventType>) {
        if !events.is_empty() && self.sender.send(Event::new(self.uuid, events)).is_err() {
            tracing::warn!("Failed to send event");
        }
    }
}

impl Connection<Tcp> {
    pub fn read(&mut self, size: u64) {
        self.state.read += size;
    }
    pub fn write(&mut self, size: u64) {
        self.state.write += size;
    }
}

impl Connection<Udp> {
    pub fn recv_from(&mut self, addr: Address, size: u64) {
        self.state
            .recv_from
            .entry(addr)
            .and_modify(|e| *e += size)
            .or_insert(size);
    }
    pub fn send_to(&mut self, addr: Address, size: u64) {
        self.state
            .send_to
            .entry(addr)
            .and_modify(|e| *e += size)
            .or_insert(size);
    }
}

impl<T: ConnType> Drop for Connection<T> {
    fn drop(&mut self) {
        let events = self.state.get_events();
        self.send(events);
        self.send(vec![EventType::CloseConnection])
    }
}
