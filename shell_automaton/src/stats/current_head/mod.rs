// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod stats_current_head_state;
use std::time::Duration;

pub use stats_current_head_state::*;

pub mod stats_current_head_actions;

mod stats_current_head_reducer;
pub use stats_current_head_reducer::stats_current_head_reducer;

mod stats_current_head_effects;
pub use stats_current_head_effects::stats_current_head_effects;

/// Time to keep `CurrentHead` stats in memory.
const PRUNE_PERIOD: Duration = Duration::from_secs(2 * 60 * 60);
