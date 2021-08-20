// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Duration;

/// Proposal is the message directed towards state machine.
///
/// By sending proposals, state can be mutated/updated.
///
/// Each proposal updates logical clock of the state machine, after
/// it has been fed to `Acceptor`.
pub trait Proposal {
    /// Time passed from last/previous proposal.
    fn time_passed(&self) -> Duration;

    /// Nullify passed time.
    ///
    /// useful for internal proposals.
    fn nullify_time_passed(&mut self);
}
