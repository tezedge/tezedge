// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::fmt;

/// Validator of the proposal, its id and voting power
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Validator<Id> {
    pub id: Id,
    pub power: u32,
}

impl<Id> fmt::Display for Validator<Id>
where
    Id: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id: {}, power: {}", self.id, self.power)
    }
}

/// Map that provides a validator for given level and round
pub trait ValidatorMap {
    type Id;

    /// the closest round where current baker is proposer
    fn proposer(&self, level: i32, round: i32) -> Option<(i32, Self::Id)>;
}
