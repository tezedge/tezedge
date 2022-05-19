// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct BakingSlot {
    pub slot: u16,
    pub timeout: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BakerBlockBakerState {
    Idle {
        time: u64,
    },
    RightsGetPending {
        time: u64,
        /// Slots for current level.
        slots: Option<Vec<u16>>,
        /// Slots for next level.
        next_slots: Option<Vec<u16>>,
    },
    RightsGetSuccess {
        time: u64,
        /// Slots for current level.
        slots: Vec<u16>,
        /// Slots for next level.
        next_slots: Vec<u16>,
    },
    NoRights {
        time: u64,
    },
    /// Waiting until current level/round times out and until it's time
    /// for us to bake a block.
    TimeoutPending {
        time: u64,
        /// Slot for current level's next round that we can bake.
        next_round: Option<BakingSlot>,
        /// Slots for next level's next round that we can bake.
        next_level: Option<BakingSlot>,
    },
    /// Previous round didn't reach the quorum, or we aren't baker of
    /// the next level and we haven't seen next level block yet, so
    /// it's time to bake next round.
    BakeNextRound {
        time: u64,
        slot: u16,
    },
    /// Previous round did reach the quorum, so bake the next level.
    BakeNextLevel {
        time: u64,
        slot: u16,
    },
}

impl BakerBlockBakerState {
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle { .. })
    }
}
