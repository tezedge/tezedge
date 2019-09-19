use std::time::Instant;
use serde::Serialize;

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeerMetrics {
    identifier: String,
    transferred_bytes: usize,
    average_transfer_speed: f32,
    current_transfer_speed: f32,
}

struct PeerMonitor {
    identifier: String,
    is_connected: bool,
    total_transferred: usize,
    current_transferred: usize,
    last_update: Instant,
    first_update: Instant,
}

impl PeerMonitor {
    pub fn new(identifier: String) -> Self {
        let now = Instant::now();
        Self {
            identifier,
            is_connected: true,
            total_transferred: 0,
            current_transferred: 0,
            last_update: now.clone(),
            first_update: now,
        }
    }

    pub fn connected(&mut self) {
        self.is_connected = true;
    }

    pub fn disconnected(&mut self) {
        self.is_connected = false;
    }

    pub fn avg_speed(&self) -> f32 {
        self.total_transferred as f32 / self.first_update.elapsed().as_secs_f32()
    }

    pub fn current_speed(&self) -> f32 {
        self.current_transferred as f32 / self.last_update.elapsed().as_secs_f32()
    }

    pub fn transferred_bytes(&self) -> usize {
        self.total_transferred
    }

    pub fn incoming_bytes(&mut self, incoming: usize) {
        self.total_transferred += incoming;
        self.current_transferred += incoming
    }

    pub fn snapshot(&mut self) -> PeerMetrics {
        let ret = PeerMetrics {
            identifier: self.identifier.clone(),
            transferred_bytes: self.total_transferred,
            average_transfer_speed: self.avg_speed(),
            current_transfer_speed: self.current_speed(),
        };

        self.current_transferred = 0;
        self.last_update = Instant::now();
        return ret;
    }
}