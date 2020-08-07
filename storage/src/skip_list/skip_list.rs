// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::max;
use std::collections::BTreeMap;
use std::hash::Hash;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::persistent::{BincodeEncoded, Codec, KeyValueSchema, KeyValueStoreWithSchema};
use crate::persistent::sequence::SequenceGenerator;
use crate::skip_list::{LEVEL_BASE, SkipListError, TryExtend};
use crate::skip_list::content::{ListValueDatabase, NodeHeader, SkipListId};
use crate::skip_list::lane::{Lane, LaneDatabase, TypedLane};

pub type SkipListDatabase = dyn KeyValueStoreWithSchema<DatabaseBackedSkipList> + Sync + Send;

/// Data structure implementation, managing structure data, shape and metadata
/// It is expected, that structure will hold a few GiBs of data, and because of that,
/// structure is backed by an database.
pub struct DatabaseBackedSkipList {
    list_db: Arc<SkipListDatabase>,
    lane_db: Arc<LaneDatabase>,
    value_db: Arc<ListValueDatabase>,
    sequence_gen: Arc<SequenceGenerator>,
    list_id: SkipListId,
    state: SkipListState,
}

impl KeyValueSchema for DatabaseBackedSkipList {
    type Key = SkipListId;
    type Value = SkipListState;

    fn name() -> &'static str {
        "skip_list"
    }
}

impl DatabaseBackedSkipList {
    /// Create new list in given database
    pub fn new(list_id: SkipListId, db: Arc<rocksdb::DB>, sequence_gen: Arc<SequenceGenerator>) -> Result<Self, SkipListError> {
        let value_db: Arc<ListValueDatabase> = db.clone();
        let lane_db: Arc<LaneDatabase> = db.clone();
        let list_db: Arc<SkipListDatabase> = db;
        let state = list_db.get(&list_id)?
            .unwrap_or_else(|| SkipListState {
                levels: 1,
                len: 0,
            });

        Ok(Self { list_db, lane_db, value_db, list_id, state, sequence_gen })
    }

    /// Find highest level, which we should traverse to hit the index
    pub fn index_level(index: usize) -> usize {
        if index == 0 {
            0
        } else {
            f64::log((index + 1) as f64, LEVEL_BASE as f64).floor() as usize
        }
    }

    fn lane(&self, level: usize) -> Lane {
        Lane::new(self.list_id, level, self.lane_db.clone(), self.value_db.clone(), self.sequence_gen.clone())
    }

    /// Rebuild state for given index
    fn get_internal<K, V>(&self, index: usize, prefix: Option<&K>) -> Result<Option<BTreeMap<K, V>>, SkipListError>
        where
            K: Ord + Codec + Hash + Eq,
            V: Codec
    {
        // There is an sequential index on lowest level, if expected index is bigger than
        // length of chain, it is not stored, otherwise, IT MUST BE FOUND.
        if index >= self.state.len {
            return Ok(None);
        }

        let mut results = Vec::with_capacity(2048);
        let mut lane = self.lane(Self::index_level(index));
        let mut pos = NodeHeader::new(self.list_id, lane.level(), 0);

        loop {
            let lane_values = match prefix {
                Some(prefix) => lane.get_prefix(pos.index(), prefix)? as Option<Vec<(K, V)>>,
                None => lane.get_all(pos.index())? as Option<Vec<(K, V)>>
            };

            if let Some(list_value_map) = lane_values {
                results.extend(list_value_map);
            } else {
                return Err(SkipListError::InternalError {
                    description: format!("Value not found in lanes, even thou it should: \
                                         current_level: {} | current_lane_index: {} | \
                                         current_index: {}",
                                         lane.level(), pos.index(), pos.base_index()),
                });
            }

            if pos.base_index() == index {
                break;
            } else {
                let mut desc = false;
                while pos.next().base_index() > index {
                    // We cannot move horizontally anymore, so we need to descent to lower level.
                    if lane.level() == 0 {
                        return Err(SkipListError::InternalError {
                            description: "Correct value was skipped".to_string(),
                        });
                    } else {
                        // Make descend
                        lane = lane.lower_lane();
                        pos = pos.lower();
                        desc = true;
                    }
                }

                if !desc {
                    pos = pos.next();
                }
            }
        }

        Ok(Some(results.into_iter().collect()))
    }
}

pub trait SkipList {
    fn len(&self) -> usize;

    fn levels(&self) -> usize;

    fn contains(&self, index: usize) -> bool;
}

impl SkipList for DatabaseBackedSkipList {
    /// Get number of elements stored in this node
    #[inline]
    fn len(&self) -> usize {
        self.state.len
    }

    #[inline]
    fn levels(&self) -> usize {
        self.state.levels
    }

    /// Check, that given index is stored in structure
    #[inline]
    fn contains(&self, index: usize) -> bool {
        self.state.len > index
    }
}

pub trait TypedSkipList<K: Codec, V: Codec>: SkipList {
    fn get(&self, index: usize) -> Result<Option<BTreeMap<K, V>>, SkipListError>;

    fn get_prefix(&self, index: usize, prefix: &K) -> Result<Option<BTreeMap<K, V>>, SkipListError>;

    fn get_key(&self, index: usize, key: &K) -> Result<Option<V>, SkipListError>;

    fn push(&mut self, value: &BTreeMap<K, V>) -> Result<(), SkipListError>;
}

impl<K, V> TypedSkipList<K, V> for DatabaseBackedSkipList
    where
        K: Ord + Codec + Hash + Eq,
        V: Codec
{
    /// Rebuild state for given index
    fn get(&self, index: usize) -> Result<Option<BTreeMap<K, V>>, SkipListError> {
        self.get_internal(index, None)
    }

    fn get_prefix(&self, index: usize, prefix: &K) -> Result<Option<BTreeMap<K, V>>, SkipListError> {
        self.get_internal(index, Some(prefix))
    }

    /// Get single value from state at given index
    fn get_key(&self, index: usize, key: &K) -> Result<Option<V>, SkipListError> {
        if index >= self.state.len {
            return Ok(None);
        }

        let highest_level = Self::index_level(index);
        let mut lane = self.lane(0);
        let mut pos = NodeHeader::new(self.list_id, lane.level(), index);

        loop {
            if let Some(value) = lane.get(pos.index(), key)? as Option<V> {
                return Ok(Some(value));
            } else {
                if pos.is_edge_node() && pos.level() < highest_level {
                    pos = pos.higher();
                    lane = lane.higher_lane();
                } else {
                    if pos.index() == 0 {
                        return Ok(None);
                    } else {
                        pos = pos.prev();
                    }
                }
            }
        }
    }

    /// Push new value into the end of the list. Beware, this is operation is
    /// not thread safe and should be handled with care !!!
    fn push(&mut self, value: &BTreeMap<K, V>) -> Result<(), SkipListError> {
        let mut lane = self.lane(0);
        let mut pos = NodeHeader::new(self.list_id, lane.level(), self.state.len);

        // Insert value into lowest level, as is.
        lane.put_list_value(pos.index())?.try_extend(value)?;

        // Start building upper lanes
        while pos.is_edge_node() {
            let pos_higher = pos.higher();
            let mut lane_higher = lane.clone().higher_lane();
            let mut list_value_higher = lane_higher.put_list_value(pos_higher.index())?;
            lane.put_list_value(pos.index())?.try_extend(value)?;

            let start = pos.index() + 1 - LEVEL_BASE;
            for index in start..=pos.index() {
                let list_value = lane.get_list_value(index)?.unwrap_or_else(|| panic!("Expected list value at: {:?}", &pos_higher));
                list_value_higher.merge(&list_value)?;
            }

            // build even higher lane (only if is edge node)
            pos = pos_higher;
            lane = lane_higher;
        }

        self.state.levels = max(lane.level() + 1, self.state.levels);
        self.state.len += 1;

        self.list_db.put(&self.list_id, &self.state)
            .map_err(SkipListError::from)
    }
}

/// This structure holds state of the skip list which will be persisted into a database.
#[derive(Serialize, Deserialize)]
pub struct SkipListState {
    /// how many levels (lanes) does skip list contain
    levels: usize,
    /// length (number of unique items) stored in skip list
    len: usize,
}

impl BincodeEncoded for SkipListState {}