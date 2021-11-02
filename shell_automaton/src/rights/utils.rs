// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use storage::cycle_eras_storage::{CycleEra, CycleErasData};
use tezos_messages::p2p::encoding::block_header::Level;

use super::Cycle;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum CycleError {
    #[error("Cycle era is not found for `{0}`")]
    EraNotFound(Level),
    #[error("Cycle `{0}` is too far from `{1}`")]
    CycleOutOfBounds(Cycle, Cycle),
    #[error("Invalid value for `{0}`")]
    InvalidValue(String),
}

fn get_cycle_era(level: Level, cycle_eras: &CycleErasData) -> Result<&CycleEra, CycleError> {
    cycle_eras
        .iter()
        .find(|era| era.first_level() <= &level)
        .ok_or_else(|| CycleError::EraNotFound(level))
}

pub type Position = i32;

fn cycle_from_level(level: Level, era: &CycleEra) -> Result<(Cycle, Position), CycleError> {
    if *era.blocks_per_cycle() > 0 {
        let cycle = (level - era.first_level()) / era.blocks_per_cycle() + era.first_cycle();
        let position = (level - era.first_level()) % era.blocks_per_cycle();
        Ok((cycle, position))
    } else {
        Err(CycleError::InvalidValue("blocks_per_cycle".to_string()))
    }
}

fn assert_cycle(cycle: Cycle, block_cycle: Cycle, preserved_cycles: u8) -> Result<(), CycleError> {
    if (block_cycle - cycle).abs() <= preserved_cycles.into() {
        Ok(())
    } else {
        Err(CycleError::CycleOutOfBounds(cycle, block_cycle))
    }
}

pub(super) fn get_cycle(
    block_level: Level,
    level: Option<Level>,
    cycle_eras: &CycleErasData,
    preserved_cycles: u8,
) -> Result<(Cycle, Position), CycleError> {
    let level = level.unwrap_or(block_level);
    let block_cycle_era = get_cycle_era(block_level, cycle_eras)?;
    let cycle_era = get_cycle_era(level, cycle_eras)?;

    let (cycle, position) = cycle_from_level(level, cycle_era)?;
    let (block_cycle, _) = cycle_from_level(block_level, block_cycle_era)?;
    assert_cycle(cycle, block_cycle, preserved_cycles)?;

    Ok((cycle, position))
}
