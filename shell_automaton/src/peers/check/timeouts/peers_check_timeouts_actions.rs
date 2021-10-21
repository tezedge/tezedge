use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use super::PeersTimeouts;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsInitAction {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsSuccessAction {
    pub peer_timeouts: PeersTimeouts,
    pub graylist_timeouts: Vec<IpAddr>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsCleanupAction {}
