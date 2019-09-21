use serde::Serialize;
use crate::monitors::PeerMonitor;
use std::iter::FromIterator;

// -------------------------- GENERAL METRICS -------------------------- //
pub type IncomingTransferMetrics = ();

// -------------------------- PEER TRANSFER STATS MESSAGE -------------------------- //
#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeerMetrics {
    identifier: String,
    transferred_bytes: usize,
    average_transfer_speed: f32,
    current_transfer_speed: f32,
}

impl PeerMetrics {
    pub fn new(identifier: String, transferred_bytes: usize, average_transfer_speed: f32, current_transfer_speed: f32) -> Self {
        Self {
            identifier,
            transferred_bytes,
            average_transfer_speed,
            current_transfer_speed,
        }
    }
}

// -------------------------- PEER CONNECTING/DISCONNECTING MESSAGE -------------------------- //
#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase", tag = "status", content = "identifier")]
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
#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Message {
    PeersMetrics {
        payload: Vec<PeerMetrics>
    },
    PeerStatus {
        payload: PeerConnectionStatus,
    },
    NotImplemented(String),
}

impl<'a> FromIterator<&'a mut PeerMonitor> for Message {
    fn from_iter<I: IntoIterator<Item=&'a mut PeerMonitor>>(monitors: I) -> Self {
        let mut payload = Vec::new();
        for monitor in monitors {
            payload.push(monitor.snapshot())
        }

        Self::PeersMetrics { payload }
    }
}

impl From<PeerConnectionStatus> for Message {
    fn from(payload: PeerConnectionStatus) -> Self {
        Self::PeerStatus { payload }
    }
}