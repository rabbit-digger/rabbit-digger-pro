use std::collections::HashMap;

use parking_lot::Mutex as SyncMutex;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::types::Response;

pub struct ClientSessionState {
    session_id: Uuid,
    seq_id: SyncMutex<u32>,
    wait_map: SyncMutex<HashMap<u32, oneshot::Sender<(Response, Vec<u8>)>>>,
}

impl ClientSessionState {
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4(),
            seq_id: SyncMutex::new(0),
            wait_map: SyncMutex::new(HashMap::new()),
        }
    }
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }
    pub fn next_seq_id(&self) -> u32 {
        let mut seq_id = self.seq_id.lock();
        *seq_id += 1;
        *seq_id
    }
    pub fn wait_for_response(&self, seq_id: u32) -> oneshot::Receiver<(Response, Vec<u8>)> {
        let (tx, rx) = oneshot::channel();
        self.wait_map.lock().insert(seq_id, tx);
        rx
    }
    pub fn send_response(&self, resp: Response, data: Vec<u8>) {
        if let Some(tx) = self.wait_map.lock().remove(&resp.seq_id) {
            let _ = tx.send((resp, data));
        }
    }
}
