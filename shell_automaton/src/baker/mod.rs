// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod block_baker;
pub mod block_endorser;

mod baker_state;
pub use baker_state::*;

mod baker_effects;
pub use baker_effects::*;

// TODO(zura): read these constants from protocol.
pub const MINIMAL_BLOCK_DELAY: u64 = 15;
pub const DELAY_INCREMENT_PER_ROUND: u64 = 5;
pub const CONSENSUS_COMMITTEE_SIZE: u32 = 7000;
pub const BLOCKS_PER_COMMITMENT: i32 = 32;
pub const PROOF_OF_WORK_THRESHOLD: u64 = 70368744177663;
