// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    pub is_bootstrapped: bool,
}
