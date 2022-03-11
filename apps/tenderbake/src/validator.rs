// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::fmt;

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

pub trait ValidatorMap {
    fn preendorser(&self, level: i32, round: i32) -> Option<Validator>;
    fn endorser(&self, level: i32, round: i32) -> Option<Validator>;
    fn proposer(&self, level: i32, round: i32) -> bool;
}
