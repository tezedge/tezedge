// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod rights_state;
use crypto::hash::BlockHash;
pub use rights_state::*;

mod rights_actions;
pub use rights_actions::*;

mod rights_reducer;
pub use rights_reducer::rights_reducer;

mod rights_effects;
pub use rights_effects::rights_effects;
use tezos_messages::p2p::encoding::block_header::Level;

mod utils;

/// Key identifying particular request for endorsing rights.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct EndorsingRightsKey {
    /// Current block hash.
    current_block_hash: BlockHash,
    /// Level of block to calculate endorsing rights for. If `None`, `current_block_hash` is used instead.
    level: Option<Level>,
}

pub use crate::storage::kv_cycle_meta::Cycle;
