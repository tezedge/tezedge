// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! This crate provides definitions of tezos messages.

use chrono::prelude::*;
use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType};

use crate::p2p::encoding::block_header::Level;

pub mod base;
pub mod p2p;
pub mod protocol;

/// Helper function to format UNIX (integral) timestamp to RFC3339 string timestamp
pub fn ts_to_rfc3339(ts: i64) -> String {
    Utc.from_utc_datetime(&NaiveDateTime::from_timestamp(ts, 0))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// This common struct holds info about head and his level
#[derive(Clone, Debug, Getters, Serialize, Deserialize)]
pub struct Head {
    /// BlockHash of head.
    #[get = "pub"]
    hash: BlockHash,
    /// Level of the head.
    #[get = "pub"]
    level: Level,
}

impl Head {
    pub fn new(hash: BlockHash, level: Level) -> Self {
        Self {
            hash,
            level,
        }
    }

    pub fn to_debug_info(&self) -> (String, Level) {
        (HashType::BlockHash.bytes_to_string(&self.hash), self.level)
    }
}

impl From<Head> for BlockHash {
    fn from(h: Head) -> Self {
        h.hash
    }
}

impl From<Head> for Level {
    fn from(h: Head) -> Self {
        h.level
    }
}