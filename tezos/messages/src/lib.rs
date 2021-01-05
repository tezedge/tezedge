// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! This crate provides definitions of tezos messages.

use chrono::prelude::*;
use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType};

use crate::p2p::encoding::block_header::{display_fitness, Fitness, Level};

pub mod base;
pub mod p2p;
pub mod protocol;

/// Helper function to format UNIX (integral) timestamp to RFC3339 string timestamp
pub fn ts_to_rfc3339(ts: i64) -> String {
    Utc.from_utc_datetime(&NaiveDateTime::from_timestamp(ts, 0))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// This common struct holds info (hash, level, fitness) about block used as head,
/// e.g. for fast computations without need to access storage
/// (if you need here more attributes from block_header, consider refactor block_header with this struct as shell_header)
#[derive(Clone, Debug, Getters, Serialize, Deserialize)]
pub struct Head {
    /// BlockHash of head.
    #[get = "pub"]
    block_hash: BlockHash,
    /// Level of the head.
    #[get = "pub"]
    level: Level,
    /// Fitness of block
    #[get = "pub"]
    fitness: Fitness,
}

impl Head {
    pub fn new(block_hash: BlockHash, level: Level, fitness: Fitness) -> Self {
        Self {
            block_hash,
            level,
            fitness,
        }
    }

    pub fn to_debug_info(&self) -> (String, Level, String) {
        (
            HashType::BlockHash.hash_to_b58check(&self.block_hash),
            self.level,
            display_fitness(&self.fitness),
        )
    }
}

impl From<Head> for BlockHash {
    fn from(h: Head) -> Self {
        h.block_hash
    }
}

impl From<Head> for Level {
    fn from(h: Head) -> Self {
        h.level
    }
}
