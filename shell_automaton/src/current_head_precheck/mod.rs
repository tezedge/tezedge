// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// TODO(zura|alexander): refactor. Move to
// `shell_automaton::current_head::precheck` module and adjust names.

mod current_head_precheck_state;
use crypto::hash::BlockHash;
pub use current_head_precheck_state::*;

mod current_head_precheck_actions;
pub use current_head_precheck_actions::*;

mod current_head_precheck_reducer;
pub use current_head_precheck_reducer::*;

mod current_head_precheck_effects;
pub use current_head_precheck_effects::*;
use tezos_messages::protocol::SupportedProtocol;

use crate::State;

/// Checks if block prechecking is supported for the block that is a successor of the block with `prev_block` hash.
fn block_prechecking_possible(state: &State, prev_block: &BlockHash) -> bool {
    state
        .prechecker
        .protocol_version_cache
        .next_protocol_versions
        .get(prev_block)
        .map_or(false, |(_, supported_protocol)| {
            matches!(
                supported_protocol,
                SupportedProtocol::Proto010 | SupportedProtocol::Proto011
            )
        })
}
