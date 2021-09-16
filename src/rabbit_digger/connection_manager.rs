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
#[serde(rename_all = "lowercase")]
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

#[derive(Debug, Serialize)]
pub struct ConnectionState {
    connections: HashMap<Uuid, ConnectionInfo>,
    total_upload: usize,
    total_download: usize,
}

impl ConnectionState {
    fn new() -> Self {
        ConnectionState {
            connections: HashMap::new(),
            total_upload: 0,
            total_download: 0,
        }
    }
    fn input_event(&mut self, event: &Event) {
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
                        upload: 0,
                        download: 0,
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
                        upload: 0,
                        download: 0,
                    },
                );
            }
            EventType::Inbound(download) => {
                if let Some(conn) = self.connections.get_mut(&uuid) {
                    conn.download += download;
                    self.total_download += download;
                }
            }
            EventType::Outbound(upload) => {
                if let Some(conn) = self.connections.get_mut(&uuid) {
                    conn.upload += upload;
                    self.total_upload += upload;
                }
            }
            EventType::UdpInbound(_, download) => {
                if let Some(conn) = self.connections.get_mut(&uuid) {
                    conn.download += download;
                    self.total_download += download;
                }
            }
            EventType::UdpOutbound(_, upload) => {
                if let Some(conn) = self.connections.get_mut(&uuid) {
                    conn.upload += upload;
                    self.total_upload += upload;
                }
            }
            EventType::CloseConnection => {
                self.connections.remove(&uuid);
            }
        };
    }
    fn input_events<'a>(&mut self, events: impl Iterator<Item = &'a Event>) {
        for event in events {
            self.input_event(event);
        }
    }
}

#[derive(Clone)]
pub struct ConnectionManager {
    state: Arc<Mutex<ConnectionState>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        let inner = Arc::new(Mutex::new(ConnectionState::new()));
        Self { state: inner }
    }
    pub fn input_event(&self, event: &Event) {
        self.state.lock().input_event(event)
    }
    pub fn input_events<'a>(&self, events: impl Iterator<Item = &'a Event>) {
        self.state.lock().input_events(events)
    }
    pub fn borrow_state<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&ConnectionState) -> R,
    {
        let conn = &self.state.lock();
        f(conn)
    }
}
