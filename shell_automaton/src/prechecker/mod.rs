// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod prechecker_state;
use crypto::hash::BlockHash;
pub use prechecker_state::*;

pub mod prechecker_actions;

mod prechecker_reducer;
pub use prechecker_reducer::prechecker_reducer;

mod prechecker_effects;
pub use prechecker_effects::prechecker_effects;

mod prechecker_validator;
pub use prechecker_validator::*;
use tezos_messages::protocol::SupportedProtocol;

use crate::State;

/// Checks if prechecking is enabled for the block that is a successor of the block with `prev_block` hash.
pub(crate) fn prechecking_enabled(state: &State, prev_block: &BlockHash) -> bool {
    !state.config.disable_endorsements_precheck
        && state
            .prechecker
            .protocol_version_cache
            .next_protocol_versions
            .get(prev_block)
            .map_or(false, |(_, supported_protocol)| match supported_protocol {
                SupportedProtocol::Proto010 | SupportedProtocol::Proto011 => true,
                _ => false,
            })
}
