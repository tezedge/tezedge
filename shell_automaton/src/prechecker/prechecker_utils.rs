// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use super::CycleEra;
use tezos_messages::p2p::encoding::block_header::Level;

pub fn get_cycle_level(level: Level, cycle_eras: &[CycleEra]) -> Option<(i32, i32)> {
    cycle_eras.iter().find_map(|era| {
        if era.first_level > level {
            return None;
        }
        let cycle = (level - era.first_level) / era.blocks_per_cycle + era.first_cycle;
        let level_in_cycle = (level - era.first_level) % era.blocks_per_cycle;
        Some((cycle, level_in_cycle))
    })
}
