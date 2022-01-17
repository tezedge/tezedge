// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{mpsc, Arc};

use crypto::hash::BlockHash;

pub use shell_automaton::service::actors_service::{
    ActorsMessageFrom, ActorsMessageTo, ActorsService,
};
use shell_automaton::service::actors_service::{ApplyBlockCallback, ApplyBlockResult};

/// Mocked ActorsService.
///
/// Does nothing.
#[derive(Debug, Clone)]
pub struct ActorsServiceDummy {}

impl ActorsServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl ActorsService for ActorsServiceDummy {
    fn send(&self, _: ActorsMessageTo) {}

    fn try_recv(&mut self) -> Result<ActorsMessageFrom, mpsc::TryRecvError> {
        Err(mpsc::TryRecvError::Empty)
    }

    fn register_apply_block_callback(&mut self, _: Arc<BlockHash>, _: ApplyBlockCallback) {}

    fn call_apply_block_callback(&mut self, _: &BlockHash, _: ApplyBlockResult) {}
}
