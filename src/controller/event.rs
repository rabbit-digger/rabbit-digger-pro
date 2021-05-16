use std::time::Instant;

use rd_interface::{Address, Arc};
use uuid::Uuid;

#[derive(Debug)]
pub enum EventType {
    NewTcp(Address),
    CloseConnection,
    Outbound(usize),
    Inbound(usize),
}

#[derive(Debug)]
pub struct Event {
    pub uuid: Uuid,
    pub event_type: EventType,
    pub time: Instant,
}

pub type BatchEvent = Vec<Arc<Event>>;

impl Event {
    pub fn new(uuid: Uuid, event_type: EventType) -> Event {
        Event {
            uuid,
            event_type,
            time: Instant::now(),
        }
    }
}
