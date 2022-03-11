// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{cmp::Ordering, collections::BTreeMap, fmt, time::Duration};

use super::{timestamp::Timestamp, validator::Validator};

#[derive(Clone, PartialEq, Eq)]
pub struct BlockId {
    pub level: i32,
    pub round: i32,
    pub payload_hash: [u8; 32],
    pub payload_round: i32,
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.level, self.round)
    }
}

impl fmt::Debug for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl PartialOrd for BlockId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.level == other.level && self.payload_hash == other.payload_hash {
            Some(self.round.cmp(&other.round))
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct Votes<I> {
    pub ids: BTreeMap<u32, I>,
    pub power: u32,
}

impl<I> Default for Votes<I> {
    fn default() -> Self {
        Votes {
            ids: BTreeMap::default(),
            power: 0,
        }
    }
}

impl<I> Eq for Votes<I> {}

impl<I> PartialEq for Votes<I> {
    fn eq(&self, other: &Self) -> bool {
        if self.ids.len() != other.ids.len() || self.power != other.power {
            return false;
        }
        for k in self.ids.keys() {
            if !other.ids.contains_key(k) {
                return false;
            }
        }

        true
    }
}

impl<I> Votes<I> {
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty() && self.power == 0
    }
}

impl<I> FromIterator<(Validator, I)> for Votes<I> {
    fn from_iter<T: IntoIterator<Item = (Validator, I)>>(iter: T) -> Self {
        let mut s = Votes::default();
        s.extend(iter);
        s
    }
}

impl<I> Extend<(Validator, I)> for Votes<I> {
    fn extend<T: IntoIterator<Item = (Validator, I)>>(&mut self, iter: T) {
        for (Validator { id, power }, op) in iter {
            self.ids.insert(id, op);
            self.power += power;
        }
    }
}

impl<I> fmt::Debug for Votes<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.power)
    }
}

#[derive(Debug, Clone)]
pub struct Prequorum<I> {
    pub block_id: BlockId,
    pub votes: Votes<I>,
}

impl<I> Eq for Prequorum<I> {}

impl<I> PartialEq for Prequorum<I> {
    fn eq(&self, other: &Self) -> bool {
        self.block_id == other.block_id && self.votes == other.votes
    }
}

impl<I> PartialOrd for Prequorum<I> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.block_id.partial_cmp(&other.block_id)
    }
}

#[derive(Debug, Clone)]
pub struct Quorum<I> {
    pub votes: Votes<I>,
}

pub trait Payload {
    type Item;

    const EMPTY: Self;

    fn update(&mut self, item: Self::Item);
}

#[derive(Clone)]
pub struct BlockInfo<P>
where
    P: Payload,
{
    pub hash: [u8; 32],
    pub block_id: BlockId,
    pub timestamp: Timestamp,
    pub transition: bool,
    pub prequorum: Option<Prequorum<P::Item>>,
    pub quorum: Option<Quorum<P::Item>>,
    pub payload: P,
}

impl<P> Default for BlockInfo<P>
where
    P: Payload,
{
    fn default() -> Self {
        BlockInfo::GENESIS
    }
}

impl<P> BlockInfo<P>
where
    P: Payload,
{
    pub const GENESIS: Self = BlockInfo {
        hash: [0; 32],
        block_id: BlockId {
            level: 0,
            round: 0,
            payload_hash: [0; 32],
            payload_round: 0,
        },
        timestamp: Timestamp {
            unix_epoch: Duration::ZERO,
        },
        transition: true,
        prequorum: None,
        quorum: None,
        payload: P::EMPTY,
    };
}

impl<P> fmt::Debug for BlockInfo<P>
where
    P: Payload,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}", self.block_id, self.timestamp)
    }
}
