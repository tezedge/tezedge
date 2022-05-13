// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::block_endorser::BakerBlockEndorserState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerState {
    pub block_endorser: BakerBlockEndorserState,
}

impl BakerState {
    pub fn new() -> Self {
        Self {
            block_endorser: BakerBlockEndorserState::Idle { time: 0 },
        }
    }
}
