use std::time::SystemTime;

use rd_interface::{Address, Value};
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Debug)]
pub enum EventType {
    NewTcp(Address, Value, oneshot::Sender<()>),
    NewUdp(Address, Value, oneshot::Sender<()>),
    CloseConnection,
    Outbound(u64),
    Inbound(u64),
    UdpOutbound(Address, u64),
    UdpInbound(Address, u64),
}

#[derive(Debug)]
pub struct Event {
    pub uuid: Uuid,
    pub event_type: EventType,
    pub time: SystemTime,
}

impl Event {
    pub fn new(uuid: Uuid, event_type: EventType) -> Event {
        Event {
            uuid,
            event_type,
            time: SystemTime::now(),
        }
    }
}
