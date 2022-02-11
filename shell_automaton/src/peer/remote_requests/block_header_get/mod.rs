// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod peer_remote_requests_block_header_get_state;
pub use peer_remote_requests_block_header_get_state::*;

mod peer_remote_requests_block_header_get_actions;
pub use peer_remote_requests_block_header_get_actions::*;

mod peer_remote_requests_block_header_get_reducer;
pub use peer_remote_requests_block_header_get_reducer::*;

mod peer_remote_requests_block_header_get_effects;
pub use peer_remote_requests_block_header_get_effects::*;

pub const MAX_PEER_REMOTE_BLOCK_HEADER_REQUESTS: usize = 64;
