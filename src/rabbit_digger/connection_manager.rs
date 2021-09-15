use std::{collections::HashMap, sync::Arc};

use super::event::{Event, EventType};
use parking_lot::Mutex;
use rd_interface::{Address, Value};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    uuid: Uuid,
    addr: Address,
    ctx: Value,
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
            uuid, event_type, ..
        } = event;

        let uuid = *uuid;
        match event_type {
            EventType::NewTcp(addr, ctx) => {
                conn.insert(
                    uuid,
                    ConnectionInfo {
                        uuid,
                        addr: addr.clone(),
                        ctx: ctx.clone(),
                    },
                );
            }
            EventType::NewUdp(addr, ctx) => {
                conn.insert(
                    uuid,
                    ConnectionInfo {
                        uuid,
                        addr: addr.clone(),
                        ctx: ctx.clone(),
                    },
                );
            }
            EventType::CloseConnection => {
                conn.remove(&uuid);
            }
            _ => {}
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
