use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use super::event::{Event, EventType};

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    sender: UnboundedSender<Event>,
    #[allow(dead_code)]
    timeout: Option<Duration>,
}

impl ConnectionConfig {
    pub fn new(sender: UnboundedSender<Event>) -> Self {
        ConnectionConfig {
            sender,
            timeout: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Connection {
    uuid: Uuid,
    config: ConnectionConfig,
}

impl Connection {
    pub fn new(config: ConnectionConfig, event_type: EventType) -> Connection {
        let uuid = Uuid::new_v4();
        let this = Connection { uuid, config };
        this.send(event_type);
        this
    }
    pub fn send(&self, event_type: EventType) {
        if self
            .config
            .sender
            .send(Event::new(self.uuid, event_type))
            .is_err()
        {
            tracing::warn!("Failed to send event");
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.send(EventType::CloseConnection)
    }
}
