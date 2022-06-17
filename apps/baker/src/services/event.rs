// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::str;

use serde::{Deserialize, Serialize};

#[cfg(feature = "fuzzing")]
use super::operation_mutator::OperationContentMutator;

use crate::tenderbake_new::hash;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct Block {
    pub hash: hash::BlockHash,
    pub level: i32,
    pub predecessor: hash::BlockHash,
    pub timestamp: u64,
    pub payload_hash: hash::BlockPayloadHash,
    pub payload_round: i32,
    pub round: i32,
    pub transition: bool,
}

/// Adapter for serialize and deserialize
#[derive(Clone, Debug)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct Slots(pub Vec<u16>);

impl Serialize for Slots {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        format!("{:?}", self.0).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Slots {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let s = String::deserialize(deserializer)?;
        let s = s.trim_start_matches('[').trim_end_matches(']');
        let mut slots = Slots(vec![]);
        for slot_str in s.split(',') {
            let slot = slot_str.trim().parse().map_err(D::Error::custom)?;
            slots.0.push(slot);
        }
        Ok(slots)
    }
}
