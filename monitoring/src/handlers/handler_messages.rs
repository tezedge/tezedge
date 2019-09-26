use serde::Serialize;
use crate::monitors::PeerMonitor;
use std::iter::FromIterator;

// -------------------------- GENERAL METRICS -------------------------- //
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IncomingTransferMetrics {
    eta: f32,
    current_block_count: usize,
    downloaded_blocks: usize,
    download_rate: f32,
}

impl IncomingTransferMetrics {
    pub fn new(eta: f32, current_block_count: usize, downloaded_blocks: usize, download_rate: f32) -> Self {
        Self {
            eta,
            current_block_count,
            downloaded_blocks,
            download_rate,
        }
    }
}

// -------------------------- PEER TRANSFER STATS MESSAGE -------------------------- //
#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeerMetrics {
    id: String,
    transferred_bytes: usize,
    average_transfer_speed: f32,
    current_transfer_speed: f32,
}

impl PeerMetrics {
    pub fn new(identifier: String, transferred_bytes: usize, average_transfer_speed: f32, current_transfer_speed: f32) -> Self {
        Self {
            id: identifier,
            transferred_bytes,
            average_transfer_speed,
            current_transfer_speed,
        }
    }
}

// -------------------------- PEER CONNECTING/DISCONNECTING MESSAGE -------------------------- //
#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase", tag = "status", content = "id")]
pub enum PeerConnectionStatus {
    Connected(String),
    Disconnected(String),
}

impl PeerConnectionStatus {
    pub fn connected(peer: String) -> Self {
        Self::Connected(peer)
    }

    pub fn disconnected(peer: String) -> Self {
        Self::Disconnected(peer)
    }
}

// -------------------------- MONITOR MESSAGE -------------------------- //
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HandlerMessage {
    PeersMetrics {
        payload: Vec<PeerMetrics>
    },
    PeerStatus {
        payload: PeerConnectionStatus,
    },
    IncomingTransfer {
        payload: IncomingTransferMetrics
    },
    NotImplemented(String),
}

impl<'a> FromIterator<&'a mut PeerMonitor> for HandlerMessage {
    fn from_iter<I: IntoIterator<Item=&'a mut PeerMonitor>>(monitors: I) -> Self {
        let mut payload = Vec::new();
        for monitor in monitors {
            payload.push(monitor.snapshot())
        }

        Self::PeersMetrics { payload }
    }
}

impl From<PeerConnectionStatus> for HandlerMessage {
    fn from(payload: PeerConnectionStatus) -> Self {
        Self::PeerStatus { payload }
    }
}

impl From<IncomingTransferMetrics> for HandlerMessage {
    fn from(payload: IncomingTransferMetrics) -> Self {
        Self::IncomingTransfer { payload }
    }
}