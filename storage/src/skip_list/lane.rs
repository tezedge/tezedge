// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::Direction;
use crate::persistent::{KeyValueStoreWithSchema, KeyValueSchema, Codec, Decoder, SchemaError};
use crate::persistent::database::{IteratorMode, IteratorWithSchema};

use crate::skip_list::content::{NodeHeader, SkipListId};
use crate::skip_list::SkipListError;
use std::marker::PhantomData;
use std::iter::Take;

pub type LaneDatabase = dyn KeyValueStoreWithSchema<Lane> + Sync + Send;

/// Lane is an way to traverse the chain.
/// Lane is just an linked list, containing all changes between nodes.
/// There should be multiple lanes, to be able to skip multiple nodes and traverse structure faster.
pub struct Lane {
    list_id: SkipListId,
    level: usize,
    db: Arc<LaneDatabase>,
}

impl Lane {
    /// Create new lane handler for given database
    pub fn new(list_id: SkipListId, level: usize, db: Arc<LaneDatabase>) -> Self {
        Lane { list_id, level, db }
    }

    /// Get level of current handler
    pub fn level(&self) -> usize { self.level }

    /// Create handler for a lane on one lower level
    pub fn lower_lane(self) -> Self {
        let level = if self.level == 0 {
            self.level
        } else {
            self.level - 1
        };

        Self { list_id: self.list_id, level, db: self.db }
    }

    /// Create handler for a lane on higher level
    pub fn higher_lane(self) -> Self {
        Self { list_id: self.list_id, level: self.level + 1, db: self.db }
    }
}

impl KeyValueSchema for Lane {
    type Key = NodeHeader;
    type Value = Vec<u8>;

    fn name() -> &'static str {
        "skip_list_lanes"
    }
}

pub trait TypedLane<C: Codec> {
    fn get(&self, index: usize) -> Result<Option<C>, SkipListError>;

    fn put(&self, index: usize, value: &C) -> Result<(), SkipListError>;

    fn base_iterator(&self, starting_index: usize) -> Result<LaneIterator<C>, SkipListError>;

    fn rev_base_iterator(&self, end_index: usize, count: usize) -> Result<SizedLaneIterator<C>, SkipListError>;
}

impl<C: Codec> TypedLane<C> for Lane {
    /// Get value from specific index (relative to this lane).
    fn get(&self, index: usize) -> Result<Option<C>, SkipListError> {
        self.db.get(&NodeHeader::new(self.list_id, self.level, index))
            .map_err(SkipListError::from)?
            .map(|value| C::decode(&value).map_err(SkipListError::from))
            .transpose()
    }

    /// Put new value on specific index of this lane, beware, that lanes should contain continuous
    /// indexes, thus some meta-structure should control and contain data about end of the lane, as
    /// we cannot guarantee correct end handling on lane level.
    fn put(&self, index: usize, value: &C) -> Result<(), SkipListError> {
        self.db.put(&NodeHeader::new(self.list_id, self.level, index), &value.encode()?)
            .map_err(SkipListError::from)
    }

    /// From starting index, iterate backwards.
    fn base_iterator(&self, starting_index: usize) -> Result<LaneIterator<C>, SkipListError> {
        let iterator = self.db.iterator(IteratorMode::From(
            &NodeHeader::new(self.list_id, self.level, starting_index), Direction::Reverse,
        )).map_err(SkipListError::from)?;
        Ok(LaneIterator(iterator, PhantomData))
    }

    /// Create iterator of items starting from index - count and going up to the index
    fn rev_base_iterator(&self, end_index: usize, count: usize) -> Result<SizedLaneIterator<C>, SkipListError> {
        if count > end_index + 1 || count == 0 {
            Err(SkipListError::OutOfBoundsError)
        } else {
            let starting_index = end_index + 1 - count;
            let iterator = self.db.iterator(IteratorMode::From(
                &NodeHeader::new(self.list_id, self.level, starting_index), Direction::Forward,
            )).map_err(SkipListError::from)?.take(count);
            Ok(SizedLaneIterator(iterator, count, PhantomData))
        }
    }
}

pub struct LaneIterator<'a, D: Decoder>(IteratorWithSchema<'a, Lane>, PhantomData<D>);

impl<'a, D: Decoder> Iterator for LaneIterator<'a, D> {
    type Item = (Result<NodeHeader, SchemaError>, Result<D, SchemaError>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
            .map(|(k, v)| (k, v.and_then(|v| D::decode(&v))))
    }
}

pub struct SizedLaneIterator<'a, D: Decoder>(Take<IteratorWithSchema<'a, Lane>>, usize, PhantomData<D>);

impl<'a, D: Decoder> Iterator for SizedLaneIterator<'a, D> {
    type Item = (Result<NodeHeader, SchemaError>, Result<D, SchemaError>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
            .map(|(k, v)| (k, v.and_then(|v| D::decode(&v))))
    }
}
