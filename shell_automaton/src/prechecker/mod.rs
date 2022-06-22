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

// pub(super) fn protocol_for_block(
//     block: &BlockHeader,
//     prechecker_state: &PrecheckerState,
// ) -> Option<SupportedProtocol> {
//     prechecker_state
//         .proto_cache
//         .get(&block.proto())
//         .or_else(|| {
//             prechecker_state
//                 .protocol_cache
//                 .get(block.predecessor())
//                 .map(|(_, _, next_protocol_hash)| next_protocol_hash)
//         })
//         .and_then(|protocol| SupportedProtocol::try_from(protocol).ok())
// }

/// Tenderbake round.
pub type Round = i32;
