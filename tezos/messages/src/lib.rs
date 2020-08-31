// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This crate provides definitions of tezos messages.

use chrono::prelude::*;
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Head {
    /// BlockHash of head.
    pub hash: BlockHash,
    /// Level of the head.
    pub level: Level,
}

impl Head {
    pub fn to_debug_info(&self) -> (String, Level) {
        (HashType::BlockHash.bytes_to_string(&self.hash), self.level)
    }
}