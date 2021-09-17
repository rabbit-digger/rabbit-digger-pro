use std::{
    sync::{atomic::Ordering, Arc},
    time::SystemTime,
};

use super::event::{Event, EventType};
use atomic_shim::AtomicU64;
use dashmap::DashMap;
use rd_interface::{Address, Value};
use serde::{Serialize, Serializer};
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
    fn input_event(&self, event: &Event) {
        let Event {
            uuid,
            event_type,
            time,
        } = event;

        let uuid = *uuid;
        match event_type {
            EventType::NewTcp(addr, ctx) => {
                self.connections.insert(
                    uuid,
                    ConnectionInfo {
                        protocol: Protocol::Tcp,
                        addr: addr.clone(),
                        ctx: ctx.clone(),
                        start_time: ts(time),
                        upload: AtomicU64::new(0),
                        download: AtomicU64::new(0),
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
                        start_time: ts(time),
                        upload: AtomicU64::new(0),
                        download: AtomicU64::new(0),
                    },
                );
            }
            EventType::Inbound(download) => {
                if let Some(conn) = self.connections.get(&uuid) {
                    conn.download.fetch_add(*download, Ordering::Relaxed);
                    self.total_download.fetch_add(*download, Ordering::Relaxed);
                }
            }
            EventType::Outbound(upload) => {
                if let Some(conn) = self.connections.get(&uuid) {
                    conn.upload.fetch_add(*upload, Ordering::Relaxed);
                    self.total_upload.fetch_add(*upload, Ordering::Relaxed);
                }
            }
            EventType::UdpInbound(_, download) => {
                if let Some(conn) = self.connections.get(&uuid) {
                    conn.download.fetch_add(*download, Ordering::Relaxed);
                    self.total_download.fetch_add(*download, Ordering::Relaxed);
                }
            }
            EventType::UdpOutbound(_, upload) => {
                if let Some(conn) = self.connections.get(&uuid) {
                    conn.upload.fetch_add(*upload, Ordering::Relaxed);
                    self.total_upload.fetch_add(*upload, Ordering::Relaxed);
                }
            }
            EventType::CloseConnection => {
                self.connections.remove(&uuid);
            }
        };
    }
    fn input_events<'a>(&self, events: impl Iterator<Item = &'a Event>) {
        for event in events {
            self.input_event(event);
        }
    }
}

#[derive(Clone)]
pub struct ConnectionManager {
    state: Arc<ConnectionState>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        let inner = Arc::new(ConnectionState::new());
        Self { state: inner }
    }
    pub fn input_events<'a>(&self, events: impl Iterator<Item = &'a Event>) {
        self.state.input_events(events)
    }
    pub fn borrow_state<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&ConnectionState) -> R,
    {
        let conn = &self.state;
        f(conn)
    }
}
