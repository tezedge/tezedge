// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt::Debug;

mod randomness_effects;
pub use randomness_effects::*;

mod effects_recorder;
pub use effects_recorder::*;

/// Effects are source of randomness.
pub trait Effects: RandomnessEffects + Debug {}

impl<T> Effects for T where T: RandomnessEffects + Debug {}

pub type DefaultEffects = rand::rngs::ThreadRng;
