use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct Hub {
    senders: Arc<DashMap<String, (Uuid, broadcast::Sender<String>)>>,
}

impl Hub {
    pub fn register(&self, device_id: &str) -> (Uuid, broadcast::Receiver<String>) {
        let connection_id = Uuid::new_v4();
        let (sender, receiver) = broadcast::channel(256);
        self.senders
            .insert(device_id.to_string(), (connection_id, sender));
        (connection_id, receiver)
    }

    pub fn send(&self, device_id: &str, message: &str) -> bool {
        self.senders
            .get(device_id)
            .map(|entry| entry.value().1.send(message.to_string()).is_ok())
            .unwrap_or(false)
    }

    pub fn unregister(&self, device_id: &str, connection_id: Uuid) {
        if let Some(entry) = self.senders.get(device_id) {
            if entry.value().0 == connection_id {
                drop(entry);
                self.senders.remove(device_id);
            }
        }
    }
}
