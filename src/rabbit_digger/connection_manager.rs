#![allow(dead_code)]
use std::{collections::HashMap, sync::Arc, time::SystemTime};

use super::event::{Event, EventType};
use parking_lot::Mutex;
use rd_interface::{Address, Value};
use serde::Serialize;
use uuid::Uuid;

fn ts(time: &SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[derive(Debug, Clone, Serialize)]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    protocol: Protocol,
    addr: Address,
    start_time: u64,
    ctx: Value,
    upload: usize,
    download: usize,
}

struct Inner {
    conn: Mutex<HashMap<Uuid, ConnectionInfo>>,
}

impl Inner {
    fn new() -> Self {
        Inner {
            conn: Mutex::new(HashMap::new()),
        }
    }
    fn _input_event(&self, conn: &mut HashMap<Uuid, ConnectionInfo>, event: &Event) {
        let Event {
            uuid,
            event_type,
            time,
        } = event;

        let uuid = *uuid;
        match event_type {
            EventType::NewTcp(addr, ctx) => {
                conn.insert(
                    uuid,
                    ConnectionInfo {
                        protocol: Protocol::Tcp,
                        addr: addr.clone(),
                        ctx: ctx.clone(),
                        start_time: ts(time),
                        upload: 0,
                        download: 0,
                    },
                );
            }
            EventType::NewUdp(addr, ctx) => {
                conn.insert(
                    uuid,
                    ConnectionInfo {
                        protocol: Protocol::Udp,
                        addr: addr.clone(),
                        ctx: ctx.clone(),
                        start_time: ts(time),
                        upload: 0,
                        download: 0,
                    },
                );
            }
            EventType::Inbound(download) => {
                if let Some(conn) = conn.get_mut(&uuid) {
                    conn.download += download;
                }
            }
            EventType::Outbound(upload) => {
                if let Some(conn) = conn.get_mut(&uuid) {
                    conn.upload += upload;
                }
            }
            EventType::UdpInbound(_, download) => {
                if let Some(conn) = conn.get_mut(&uuid) {
                    conn.download += download;
                }
            }
            EventType::UdpOutbound(_, upload) => {
                if let Some(conn) = conn.get_mut(&uuid) {
                    conn.upload += upload;
                }
            }
            EventType::CloseConnection => {
                conn.remove(&uuid);
            }
        };
    }
    fn input_event(&self, event: &Event) {
        self._input_event(&mut *self.conn.lock(), event)
    }
    fn input_events<'a>(&self, events: impl Iterator<Item = &'a Event>) {
        let conn = &mut *self.conn.lock();
        for event in events {
            self._input_event(conn, event);
        }
    }
}

#[derive(Clone)]
pub struct ConnectionManager {
    inner: Arc<Inner>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        let inner = Arc::new(Inner::new());
        Self { inner }
    }
    pub fn input_event(&self, event: &Event) {
        self.inner.input_event(event)
    }
    pub fn input_events<'a>(&self, events: impl Iterator<Item = &'a Event>) {
        self.inner.input_events(events)
    }
    pub fn borrow_connection<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&HashMap<Uuid, ConnectionInfo>) -> R,
    {
        let conn = &*self.inner.conn.lock();
        f(conn)
    }
}
