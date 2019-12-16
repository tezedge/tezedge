// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::Direction;
use crate::persistent::{KeyValueStoreWithSchema, KeyValueSchema, Codec};
use crate::persistent::database::{IteratorMode, IteratorWithSchema};

use crate::skip_list::content::{ListValue, NodeHeader, SkipListId};
use crate::skip_list::SkipListError;

pub type LaneDatabase<K, V, C> = dyn KeyValueStoreWithSchema<Lane<K, V, C>> + Sync + Send;

/// Lane is an way to traverse the chain.
/// Lane is just an linked list, containing all changes between nodes.
/// There should be multiple lanes, to be able to skip multiple nodes and traverse structure faster.
pub struct Lane<K: Codec, V: Codec, C: ListValue<K, V>> {
    list_id: SkipListId,
    level: usize,
    db: Arc<LaneDatabase<K,V,C>>,
}

impl<K: Codec, V: Codec, C: ListValue<K, V>> KeyValueSchema for Lane<K, V, C> {
    type Key = NodeHeader;
    type Value = C;

    fn name() -> &'static str {
        "skip_list_lanes"
    }
}

impl<K: Codec, V: Codec, C: ListValue<K, V>> Lane<K, V, C> {
    /// Create new lane handler for given database
    pub fn new(list_id: SkipListId, level: usize, db: Arc<LaneDatabase<K, V, C>>) -> Self {
        Self { list_id, level, db }
    }

    /// Create handler for a lane on one lower level
    pub fn lower_lane(self) -> Self {
        let level = if self.level == 0 {
            self.level
        } else {
            self.level - 1
        };

        Self::new(self.list_id, level, self.db)
    }

    /// Create handler for a lane on higher level
    pub fn higher_lane(self) -> Self { Self::new(self.list_id, self.level + 1, self.db) }

    /// Get level of current handler
    pub fn level(&self) -> usize { self.level }

    /// Get value from specific index (relative to this lane).
    pub fn get(&self, index: usize) -> Result<Option<C>, SkipListError> {
        self.db.get(&NodeHeader::new(self.list_id, self.level, index))
            .map_err(SkipListError::from)
    }

    /// Put new value on specific index of this lane, beware, that lanes should contain continuous
    /// indexes, thus some meta-structure should control and contain data about end of the lane, as
    /// we cannot guarantee correct end handling on lane level.
    pub fn put(&self, index: usize, value: &C) -> Result<(), SkipListError> {
        self.db.put(&NodeHeader::new(self.list_id, self.level, index), &value)
            .map_err(SkipListError::from)
    }

    /// From starting index, iterate backwards.
    pub fn base_iterator(&self, starting_index: usize) -> Result<IteratorWithSchema<Lane<K, V, C>>, SkipListError> {
        self.db.iterator(IteratorMode::From(
            &NodeHeader::new(self.list_id, self.level, starting_index), Direction::Reverse,
        )).map_err(SkipListError::from)
    }
}
