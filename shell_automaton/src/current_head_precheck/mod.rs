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
        .protocol_cache
        .get(prev_block)
        .map_or(false, |(_, _, next_protocol)| {
            matches!(
                SupportedProtocol::try_from(next_protocol),
                Ok(SupportedProtocol::Proto010) | Ok(SupportedProtocol::Proto011)
            )
        })
}

fn block_prechecking_enabled(state: &State) -> bool {
    !state.config.disable_block_precheck
}
