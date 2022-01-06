// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::Serialize;

/// Struct for the delegates and they voting power (in rolls)
#[derive(Serialize, Debug, Clone, Getters, Eq, Ord, PartialEq, PartialOrd)]
pub struct VoteListings {
    /// Public key hash (address, e.g tz1...)
    #[get = "pub"]
    pkh: String,

    /// Number of rolls the pkh owns
    #[get = "pub"]
    rolls: i32,
}

impl VoteListings {
    /// Simple constructor to construct VoteListings
    pub fn new(pkh: String, rolls: i32) -> Self {
        Self { pkh, rolls }
    }
}
