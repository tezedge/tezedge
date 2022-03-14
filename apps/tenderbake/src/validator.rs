// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::fmt;

/// Validator of the proposal, its id and voting power
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Validator {
    pub id: u32,
    pub power: u32,
}

impl fmt::Display for Validator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id: {}, power: {}", self.id, self.power)
    }
}

/// Map that provides a validator for given level and round
pub trait ValidatorMap {
    /// who can cast preendorsement for level and round
    fn preendorser(&self, level: i32, round: i32) -> Option<Validator>;
    /// who can cast endorsement for level and round
    fn endorser(&self, level: i32, round: i32) -> Option<Validator>;
    /// the closest round where current baker is proposer
    fn proposer(&self, level: i32, round: i32) -> Option<i32>;
}
