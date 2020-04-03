// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

// TODO: refactor binary_message to base package

//! This crate provides definitions of tezos messages.

use chrono::prelude::*;

pub mod base;
pub mod p2p;
pub mod protocol;

/// Helper function to format UNIX (integral) timestamp to RFC3339 string timestamp
pub fn ts_to_rfc3339(ts: i64) -> String {
    Utc.from_utc_datetime(&NaiveDateTime::from_timestamp(ts, 0))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}