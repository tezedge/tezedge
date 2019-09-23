use std::time::Instant;
use crate::messages::PeerMetrics;

/// General statistics about incoming transfer
///
#[allow(dead_code)]
pub(crate) struct IncomingTransferMonitor {}


/// Peer specific details about transfer *FROM* peer.
pub(crate) struct PeerMonitor {
    pub identifier: String,
    pub total_transferred: usize,
    current_transferred: usize,
    last_update: Instant,
    first_update: Instant,
}

impl PeerMonitor {
    pub fn new(identifier: String) -> Self {
        let now = Instant::now();
        Self {
            identifier,
            total_transferred: 0,
            current_transferred: 0,
            last_update: now.clone(),
            first_update: now,
        }
    }

    pub fn avg_speed(&self) -> f32 {
        self.total_transferred as f32 / self.first_update.elapsed().as_secs_f32()
    }

    pub fn current_speed(&self) -> f32 {
        self.current_transferred as f32 / self.last_update.elapsed().as_secs_f32()
    }

    #[allow(dead_code)]
    pub fn transferred_bytes(&self) -> usize {
        self.total_transferred
    }

    pub fn incoming_bytes(&mut self, incoming: usize) {
        self.total_transferred += incoming;
        self.current_transferred += incoming
    }

    pub fn snapshot(&mut self) -> PeerMetrics {
        let ret = PeerMetrics::new(
            self.identifier.clone(),
            self.total_transferred,
            self.avg_speed(),
            self.current_speed(),
        );

        self.current_transferred = 0;
        self.last_update = Instant::now();
        ret
    }
}