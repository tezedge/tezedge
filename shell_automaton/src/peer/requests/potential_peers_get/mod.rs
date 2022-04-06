// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod peer_requests_potential_peers_get_state;
pub use peer_requests_potential_peers_get_state::*;

mod peer_requests_potential_peers_get_actions;
pub use peer_requests_potential_peers_get_actions::*;

mod peer_requests_potential_peers_get_reducer;
pub use peer_requests_potential_peers_get_reducer::*;

mod peer_requests_potential_peers_get_effects;
pub use peer_requests_potential_peers_get_effects::*;

/// 3 seconds.
pub const PEER_POTENTIAL_PEERS_GET_TIMEOUT: u64 = 3 * 1_000_000_000;

/// 60 seconds.
pub const MIN_PEER_POTENTIAL_PEERS_GET_DELAY: u64 = 60 * 1_000_000_000;
