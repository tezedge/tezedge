// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::hash::Hash;
use std::sync::Arc;

use crate::persistent::sequence::SequenceGenerator;
use crate::persistent::{Codec, KeyValueSchema, KeyValueStoreWithSchema};
use crate::skip_list::content::{ListValueDatabase, NodeHeader, SkipListId, TypedListValue};
use crate::skip_list::{ListValue, SkipListError};

pub type LaneDatabase = dyn KeyValueStoreWithSchema<Lane> + Sync + Send;

/// Lane is an way to traverse the chain.
/// Lane is just an linked list, containing all changes between nodes.
/// There should be multiple lanes, to be able to skip multiple nodes and traverse structure faster.
#[derive(Clone)]
pub struct Lane {
    list_id: SkipListId,
    level: usize,
    lane_db: Arc<LaneDatabase>,
    value_db: Arc<ListValueDatabase>,
    sequence_gen: Arc<SequenceGenerator>,
}

impl Lane {
    /// Create new lane handler for given database
    pub fn new(
        list_id: SkipListId,
        level: usize,
        lane_db: Arc<LaneDatabase>,
        value_db: Arc<ListValueDatabase>,
        sequence_gen: Arc<SequenceGenerator>,
    ) -> Self {
        Lane {
            list_id,
            level,
            lane_db,
            value_db,
            sequence_gen,
        }
    }

    fn new_level(self, level: usize) -> Self {
        Self {
            list_id: self.list_id,
            level,
            lane_db: self.lane_db,
            value_db: self.value_db,
            sequence_gen: self.sequence_gen,
        }
    }

    /// Get level of current handler
    pub fn level(&self) -> usize {
        self.level
    }

    /// Create handler for a lane on one lower level
    pub fn lower_lane(self) -> Self {
        let level = if self.level == 0 {
            self.level
        } else {
            self.level - 1
        };

        self.new_level(level)
    }

    /// Create handler for a lane on higher level
    pub fn higher_lane(self) -> Self {
        let new_level = self.level + 1;
        self.new_level(new_level)
    }

    fn node_header(&self, index: usize) -> NodeHeader {
        NodeHeader::new(self.list_id, self.level, index)
    }

    pub fn get_list_value(&self, index: usize) -> Result<Option<ListValue>, SkipListError> {
        self.lane_db
            .get(&self.node_header(index))
            .map(|value_id| {
                value_id.map(|value_id| ListValue::new(value_id, self.value_db.clone()))
            })
            .map_err(SkipListError::from)
    }

    pub fn put_list_value(&mut self, index: usize) -> Result<ListValue, SkipListError> {
        let value_id = match self.lane_db.get(&self.node_header(index))? {
            Some(value_id) => value_id,
            None => {
                // generate new unique value_id
                let value_id = self.sequence_gen.next()? as usize;
                self.lane_db.put(&self.node_header(index), &value_id)?;
                value_id
            }
        };

        Ok(ListValue::new(value_id, self.value_db.clone()))
    }
}

impl KeyValueSchema for Lane {
    type Key = NodeHeader;
    type Value = usize;

    fn name() -> &'static str {
        "skip_list_lanes"
    }
}

pub trait TypedLane<K, V> {
    /// Get a single key from specific index.
    fn get(&self, index: usize, key: &K) -> Result<Option<V>, SkipListError>;

    /// Get all keys starting with `prefix`.
    fn get_prefix(&self, index: usize, prefix: &K) -> Result<Option<Vec<(K, V)>>, SkipListError>;

    /// Get values from specific index (relative to this lane).
    fn get_all(&self, index: usize) -> Result<Option<Vec<(K, V)>>, SkipListError>;
}

impl<K, V> TypedLane<K, V> for Lane
where
    K: Codec + Hash + Eq,
    V: Codec,
{
    fn get(&self, index: usize, key: &K) -> Result<Option<V>, SkipListError> {
        self.get_list_value(index)?
            .map(|list_value| list_value.get(key))
            .transpose()
            .map(|value| value.flatten())
    }

    fn get_prefix(&self, index: usize, prefix: &K) -> Result<Option<Vec<(K, V)>>, SkipListError> {
        self.get_list_value(index)?
            .map(|list_value| {
                list_value
                    .iter_prefix(prefix)
                    .and_then(|i| i.collect::<Result<Vec<(K, V)>, SkipListError>>())
            })
            .transpose()
    }

    fn get_all(&self, index: usize) -> Result<Option<Vec<(K, V)>>, SkipListError> {
        self.get_list_value(index)?
            .map(|list_value| {
                list_value
                    .iter()
                    .and_then(|i| i.collect::<Result<Vec<(K, V)>, SkipListError>>())
            })
            .transpose()
    }
}
