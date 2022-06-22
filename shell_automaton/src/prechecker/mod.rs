// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod prechecker_state;
pub use prechecker_state::*;

pub mod prechecker_actions;

mod prechecker_reducer;
pub use prechecker_reducer::prechecker_reducer;

mod prechecker_effects;
pub use prechecker_effects::prechecker_effects;

mod prechecker_validator;
pub use prechecker_validator::*;

mod operation_contents;
pub use operation_contents::*;
use tezos_messages::protocol::SupportedProtocol;

fn supported_protocols() -> Vec<SupportedProtocol> {
    vec![SupportedProtocol::Proto012, SupportedProtocol::Proto013]
}

/// Tenderbake round.
pub type Round = i32;
