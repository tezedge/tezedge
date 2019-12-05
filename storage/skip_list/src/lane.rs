// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::DB;
use serde::export::PhantomData;

use storage::Direction;
use storage::persistent::{DatabaseWithSchema, KeyValueSchema};
use storage::persistent::database::{
    IteratorMode, IteratorWithSchema,
};

use crate::content::{ListValue, NodeHeader};

type LaneDatabase<T> = dyn DatabaseWithSchema<Lane<T>> + Sync + Send;

/// Lane is an way to traverse the chain.
/// Lane is just an linked list, containing all changes between nodes.
/// There should be multiple lanes, to be able to skip multiple nodes and traverse structure faster.
#[derive(Clone, Debug)]
pub struct Lane<C: ListValue> {
    level: usize,
    db: Arc<DB>,
    _pd: PhantomData<C>,
}

impl<C: ListValue> KeyValueSchema for Lane<C> {
    type Key = NodeHeader;
    type Value = C;

    fn name() -> &'static str {
        "skip_list_lanes"
    }
}

impl<C: ListValue> Lane<C> {
    /// Create new lane handler for given database
    pub fn new(level: usize, db: Arc<DB>) -> Self {
        Self {
            level,
            db,
            _pd: PhantomData,
        }
    }

    /// Create handler for a lane on one lower level
    pub fn lower_lane(self) -> Self {
        Self::new(if self.level == 0 {
            self.level
        } else {
            self.level - 1
        }, self.db)
    }

    /// Create handler for a lane on higher level
    pub fn higher_lane(self) -> Self { Self::new(self.level + 1, self.db) }

    /// Get level of current handler
    pub fn level(&self) -> usize { self.level }

    /// Get value from specific index (relative to this lane).
    pub fn get(&self, index: usize) -> Option<C> {
        self.container().get(&NodeHeader::new(self.level, index)).unwrap()
    }

    /// Put new value on specific index of this lane, beware, that lanes should contain continuous
    /// indexes, thus some meta-structure should control and contain data about end of the lane, as
    /// we cannot guarantee correct end handling on lane level.
    pub fn put(&self, index: usize, value: &C) {
        self.container().put(&NodeHeader::new(self.level, index), &value).unwrap();
    }

    /// From starting index, iterate backwards.
    pub fn base_iterator(&self, starting_index: usize) -> Option<IteratorWithSchema<Lane<C>>> {
        self.container().iterator(IteratorMode::From(
            &NodeHeader::new(self.level, starting_index), Direction::Reverse,
        )).ok()
    }

    #[inline]
    fn container(&self) -> &Arc<impl DatabaseWithSchema<Lane<C>>> { &self.db }
}
