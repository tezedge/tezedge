// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{cmp::Ordering, fmt, time::Duration, ops::AddAssign};
use alloc::collections::BTreeMap;

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
        for v in iter {
            *self += v;
        }
    }
}

impl<I> AddAssign<(Validator, I)> for Votes<I> {
    fn add_assign(&mut self, (Validator { id, power }, op): (Validator, I)) {
        if self.ids.insert(id, op).is_none() {
            self.power += power;
        }
    }
}

#[derive(Clone)]
pub struct Prequorum<I> {
    pub block_id: BlockId,
    pub votes: Votes<I>,
}

impl<I> Eq for Prequorum<I> {}

impl<I> PartialEq for Prequorum<I> {
    fn eq(&self, other: &Self) -> bool {
        self.block_id.eq(&other.block_id)
    }
}

impl<I> PartialOrd for Prequorum<I> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.block_id.payload_hash != other.block_id.payload_hash {
            None
        } else {
            self.block_id
                .payload_round
                .partial_cmp(&other.block_id.payload_round)
        }
    }
}

#[derive(Clone)]
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

impl<P> fmt::Display for BlockInfo<P>
where
    P: Payload,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}", self.block_id, self.timestamp)
    }
}
