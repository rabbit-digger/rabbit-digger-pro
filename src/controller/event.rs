use std::time::SystemTime;

use rd_interface::{Address, Arc};
use serde_derive::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub enum EventType {
    NewTcp(Address),
    CloseConnection,
    Outbound(usize),
    Inbound(usize),
}

#[derive(Debug, Serialize)]
pub struct Event {
    pub uuid: Uuid,
    pub event_type: EventType,
    pub time: SystemTime,
}

pub type BatchEvent = Vec<Arc<Event>>;

impl Event {
    pub fn new(uuid: Uuid, event_type: EventType) -> Event {
        Event {
            uuid,
            event_type,
            time: SystemTime::now(),
        }
    }
}
