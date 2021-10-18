use serde::{Deserialize, Serialize};

use super::PeersTimeouts;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsInitAction {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsSuccessAction {
    pub timeouts: PeersTimeouts,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsCleanupAction {}
