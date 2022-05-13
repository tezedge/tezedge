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
use tezos_messages::protocol::SupportedProtocol;

mod operation_contents;
pub use operation_contents::*;

/// Checks if prechecking is enabled for the block that is a successor of the block with `prev_block` hash.
pub(crate) fn prechecking_enabled(state: &PrecheckerState, proto: u8) -> bool {
    state.proto_cache.get(&proto).map_or(false, |protocol| {
        matches!(
            protocol,
            SupportedProtocol::Proto010 | SupportedProtocol::Proto011 | SupportedProtocol::Proto012
        )
    })
}

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
