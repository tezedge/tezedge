// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use crypto::blake2b;
use storage::{cycle_storage::CycleData, num_from_slice};
use tezos_messages::p2p::encoding::block_header::Level;

use super::{
    cycle_eras::{CycleEra, CycleEras},
    Cycle, Delegate,
};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum CycleError {
    #[error("Cycle era is not found for `{0}`")]
    EraNotFound(Level),
    #[error("Cycle `{0}` is too far from `{1}`")]
    CycleOutOfBounds(Cycle, Cycle),
    #[error("Invalid value for `{0}`")]
    InvalidValue(String),
}

fn get_cycle_era(level: Level, cycle_eras: &CycleEras) -> Result<&CycleEra, CycleError> {
    cycle_eras
        .iter()
        .find(|era| era.first_level <= level)
        .ok_or(CycleError::EraNotFound(level))
}

pub type Position = i32;

fn cycle_from_level(level: Level, era: &CycleEra) -> Result<(Cycle, Position), CycleError> {
    if era.blocks_per_cycle > 0 {
        let cycle = (level - era.first_level) / era.blocks_per_cycle + era.first_cycle;
        let position = (level - era.first_level) % era.blocks_per_cycle;
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
    cycle_eras: &CycleEras,
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

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(thiserror::Error, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TezosPRNGError {
    #[error("Digest error: `{0}`")]
    Hash(#[from] blake2b::Blake2bError),
    #[error("Bounds error: `{0}`")]
    Bounds(String),
}

struct TezosPRNG {
    state: Vec<u8>,
}

impl TezosPRNG {
    fn initialize(
        seed_bytes: &[u8],
        nonce_size: usize,
        use_: &[u8],
        cycle_position: i32,
        offset: i32,
    ) -> Result<Self, TezosPRNGError> {
        let zero_bytes = vec![0; nonce_size];
        let cycle_position_bytes = cycle_position.to_be_bytes();

        // take the state (initially the random seed), zero bytes, the use string and the blocks position in the cycle as bytes, merge them together and hash the result
        let mut rd = Self::digest(&[
            seed_bytes,
            &zero_bytes,
            b"level ",
            use_,
            b":",
            &cycle_position_bytes,
        ])?;

        // set the 4 highest bytes to the result of the xor operation with offset
        let offset_bytes = offset.to_be_bytes();
        for i in 0..offset_bytes.len() {
            rd[i] ^= offset_bytes[i];
        }

        Ok(Self {
            state: Self::digest(&[&rd])?,
        })
    }

    fn get_next(&mut self, bound: i32) -> Result<i32, TezosPRNGError> {
        if bound < 1 {
            return Err(TezosPRNGError::Bounds(format!(
                "bound is negative: `{bound}`"
            )));
        }
        let num = loop {
            self.state = Self::digest(&[&self.state])?;
            // 4 highest bytes
            let r = num_from_slice!(self.state, 0, i32).abs();

            if r >= i32::MAX - (i32::MAX % bound) {
                continue;
            } else {
                break r % bound;
            }
        };

        Ok(num)
    }

    fn digest(data: &[&[u8]]) -> Result<Vec<u8>, TezosPRNGError> {
        Ok(blake2b::digest_all(data, 32)?)
    }
}

pub(super) fn random_owner(
    nonce_length: u8,
    cycle_meta_data: &CycleData,
    rolls_map: &BTreeMap<i32, Delegate>,
    use_: &[u8],
    cycle_position: i32,
    offset: impl Into<i32>,
) -> Result<Delegate, TezosPRNGError> {
    let mut prng = TezosPRNG::initialize(
        cycle_meta_data.seed_bytes(),
        nonce_length.into(),
        use_,
        cycle_position,
        offset.into(),
    )?;

    let owner = loop {
        let next = prng.get_next(*cycle_meta_data.last_roll())?;
        if let Some(delegate) = rolls_map.get(&next) {
            break delegate.clone();
        }
    };

    Ok(owner)
}

pub(super) fn endorser_rights_owner(
    nonce_length: u8,
    cycle_meta_data: &CycleData,
    rolls_map: &BTreeMap<i32, Delegate>,
    cycle_position: i32,
    slot: u16,
) -> Result<Delegate, TezosPRNGError> {
    random_owner(
        nonce_length,
        cycle_meta_data,
        rolls_map,
        b"endorsement",
        cycle_position,
        slot,
    )
}

pub(super) fn baking_rights_owner(
    nonce_length: u8,
    cycle_meta_data: &CycleData,
    rolls_map: &BTreeMap<i32, Delegate>,
    cycle_position: i32,
    priority: u16,
) -> Result<Delegate, TezosPRNGError> {
    random_owner(
        nonce_length,
        cycle_meta_data,
        rolls_map,
        b"baking",
        cycle_position,
        priority,
    )
}
