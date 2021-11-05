// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::mpsc;

pub use shell_automaton::service::actors_service::{
    ActorsMessageFrom, ActorsMessageTo, ActorsService,
};

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
}
