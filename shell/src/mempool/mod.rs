// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{Arc, RwLock};

use crate::mempool::mempool_state::MempoolState;

pub mod mempool_channel;
pub mod mempool_prevalidator;
pub mod mempool_state;

/// In-memory synchronized struct for sharing between threads/actors
pub type CurrentMempoolStateStorageRef = Arc<RwLock<MempoolState>>;

/// Inits empty mempool state storage
pub fn init_mempool_state_storage() -> CurrentMempoolStateStorageRef {
    Arc::new(RwLock::new(MempoolState::default()))
}
