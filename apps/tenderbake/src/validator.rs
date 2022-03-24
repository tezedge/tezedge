// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{fmt, ops::AddAssign};
use alloc::collections::BTreeMap;

/// Represents a pre-vote or vote
#[derive(Clone)]
pub struct Validator<Id, Op> {
    // id of the validator
    pub id: Id,
    // voting power (stake)
    pub power: u32,
    pub operation: Op,
}

impl<Id, Op> fmt::Display for Validator<Id, Op>
where
    Id: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id: {}, power: {}", self.id, self.power)
    }
}

#[derive(Clone)]
pub struct Votes<Id, Op> {
    pub ids: BTreeMap<Id, Op>,
    pub power: u32,
}

impl<Id, Op> Default for Votes<Id, Op> {
    fn default() -> Self {
        Votes {
            ids: BTreeMap::default(),
            power: 0,
        }
    }
}

impl<Id, Op> AddAssign<Validator<Id, Op>> for Votes<Id, Op>
where
    Id: Ord,
{
    fn add_assign(&mut self, v: Validator<Id, Op>) {
        if self.ids.insert(v.id, v.operation).is_none() {
            self.power += v.power;
        }
    }
}

impl<Id, Op> Extend<Validator<Id, Op>> for Votes<Id, Op>
where
    Id: Ord,
{
    fn extend<T: IntoIterator<Item = Validator<Id, Op>>>(&mut self, iter: T) {
        for v in iter {
            *self += v;
        }
    }
}

impl<Id, Op> FromIterator<Validator<Id, Op>> for Votes<Id, Op>
where
    Id: Ord,
{
    fn from_iter<T: IntoIterator<Item = Validator<Id, Op>>>(iter: T) -> Self {
        let mut s = Votes::default();
        s.extend(iter);
        s
    }
}

/// Map that provides a proposer for given level and round
pub trait ProposerMap {
    type Id;

    /// the closest round where we have proposer
    fn proposer(&self, level: i32, round: i32) -> Option<(i32, Self::Id)>;
}
