// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod rights_cycle_eras_state;
pub use rights_cycle_eras_state::*;

pub mod rights_cycle_eras_actions;

mod rights_cycle_eras_reducer;
pub use rights_cycle_eras_reducer::*;

mod rights_cycle_eras_effects;
pub use rights_cycle_eras_effects::*;
use tezos_encoding::nom::NomReader;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, NomReader)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct CycleEra {
    pub first_level: i32,
    pub first_cycle: i32,
    pub blocks_per_cycle: i32,
    pub blocks_per_commitment: i32,
}

impl From<&storage::cycle_eras_storage::CycleEra> for CycleEra {
    fn from(source: &storage::cycle_eras_storage::CycleEra) -> Self {
        Self {
            first_level: *source.first_level(),
            first_cycle: *source.first_cycle(),
            blocks_per_cycle: *source.blocks_per_cycle(),
            blocks_per_commitment: *source.blocks_per_commitment(),
        }
    }
}

pub type CycleEras = Vec<CycleEra>;
