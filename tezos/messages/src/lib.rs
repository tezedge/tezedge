// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

//! This crate provides definitions of tezos messages.

use getset::Getters;
use serde::{Deserialize, Serialize};
use time::{error::ComponentRange, format_description::well_known::Rfc3339, OffsetDateTime};

use crypto::hash::BlockHash;

use crate::p2p::encoding::block_header::{display_fitness, Fitness, Level};

pub mod base;
pub mod p2p;
pub mod protocol;

#[derive(Debug, thiserror::Error)]
#[error("invalid or out-of-range datetime")]
pub struct TimestampOutOfRangeError;

impl From<ComponentRange> for TimestampOutOfRangeError {
    fn from(_error: ComponentRange) -> Self {
        Self
    }
}

/// Helper function to format UNIX (integral) timestamp to RFC3339 string timestamp
pub fn ts_to_rfc3339(ts: i64) -> Result<String, TimestampOutOfRangeError> {
    Ok(OffsetDateTime::from_unix_timestamp(ts)?
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::from("invalid timestamp")))
}

/// This common struct holds info (hash, level, fitness) about block used as head,
/// e.g. for fast computations without need to access storage
/// (if you need here more attributes from block_header, consider refactor block_header with this struct as shell_header)
#[derive(Clone, Debug, Getters, Serialize, Deserialize)]
pub struct Head {
    // TODO: TE-369 - Arc refactor
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
            self.block_hash.to_base58_check(),
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
