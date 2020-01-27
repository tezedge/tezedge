// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::max;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::persistent::{BincodeEncoded, KeyValueStoreWithSchema, KeyValueSchema, Codec};

use crate::skip_list::{content::{ListValue, NodeHeader}, lane::Lane, lane::TypedLane, LEVEL_BASE, SkipListError};
use crate::skip_list::content::SkipListId;
use crate::skip_list::lane::LaneDatabase;

pub type SkipListDatabase = dyn KeyValueStoreWithSchema<DatabaseBackedSkipList> + Sync + Send;

/// Data structure implementation, managing structure data, shape and metadata
/// It is expected, that structure will hold a few GiBs of data, and because of that,
/// structure is backed by an database.
pub struct DatabaseBackedSkipList {
    lane_db: Arc<LaneDatabase>,
    list_db: Arc<SkipListDatabase>,
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
    pub fn new(list_id: SkipListId, db: Arc<rocksdb::DB>) -> Result<Self, SkipListError> {
        let list_db: Arc<SkipListDatabase> = db.clone();
        let state = list_db.get(&list_id)?
            .unwrap_or_else(|| SkipListState {
                levels: 1,
                len: 0,
            });

        Ok(Self { lane_db: db, list_db, list_id, state })
    }

    /// Find highest level, which we should traverse to hit the index
    pub fn index_level(index: usize) -> usize {
        if index == 0 {
            0
        } else {
            f64::log((index + 1) as f64, LEVEL_BASE as f64).floor() as usize
        }
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

pub trait TypedSkipList<K: Codec, V: Codec, C: ListValue<K, V>>: SkipList {
    fn get(&self, index: usize) -> Result<Option<C>, SkipListError>;

    fn get_key(&self, index: usize, key: &K) -> Result<Option<V>, SkipListError>;

    fn push(&mut self, value: C) -> Result<(), SkipListError>;

    fn diff(&self, from: usize, to: usize) -> Result<Option<C>, SkipListError>;
}

impl<K: Codec, V: Codec, C: ListValue<K, V>> TypedSkipList<K, V, C> for DatabaseBackedSkipList {
    /// Rebuild state for given index
    fn get(&self, index: usize) -> Result<Option<C>, SkipListError> {
        // There is an sequential index on lowest level, if expected index is bigger than
        // length of chain, it is not stored, otherwise, IT MUST BE FOUND.
        if index >= self.state.len {
            return Ok(None);
        }

        let mut current_state: Option<C> = None;
        let mut lane = Lane::new(self.list_id, Self::index_level(index), self.lane_db.clone());
        let mut pos = NodeHeader::new(self.list_id, lane.level(), 0);

        loop {
            if let Some(value) = lane.get(pos.index())? {
                if let Some(mut state) = current_state {
                    state.merge(&value);
                    current_state = Some(state);
                } else {
                    current_state = Some(value);
                }
            } else {
                panic!("Value not found in lanes, even thou it should: \
                current_level: {} | current_lane_index: {} | current_index: {}",
                       lane.level(), pos.index(), pos.base_index());
            }

            if pos.base_index() == index {
                return Ok(current_state);
            } else {
                while pos.next().base_index() > index {
                    // We cannot move horizontally anymore, so we need to descent to lower level.
                    if lane.level() == 0 {
                        panic!("Correct value was skipped");
                    } else {
                        // Make descend
                        lane = lane.lower_lane();
                        pos = pos.lower();
                    }
                }

                pos = pos.next();
            }
        }
    }

    /// Get single value from state at given index
    fn get_key(&self, index: usize, key: &K) -> Result<Option<V>, SkipListError> {
        if index >= self.state.len {
            return Ok(None);
        }

        let highest_level = Self::index_level(index);
        let mut lane = Lane::new(self.list_id, 0, self.lane_db.clone());
        let mut pos = NodeHeader::new(self.list_id, lane.level(), index);

        loop {
            if let Some(state) = lane.get(pos.index())? as Option<C> {
                if let Some(value) = state.get(key) {
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
            } else {
                panic!("Value not found in lanes, even thou it should: \
                current_level: {} | current_lane_index: {} | current_index: {}",
                       lane.level(), pos.index(), pos.base_index());
            }
        }
    }

    /// Push new value into the end of the list. Beware, this is operation is
    /// not thread safe and should be handled with care !!!
    fn push(&mut self, mut value: C) -> Result<(), SkipListError> {
        let mut lane = Lane::new(self.list_id, 0, self.lane_db.clone());
        let mut index = self.state.len;

        // Insert value into lowest level, as is.
        lane.put(index, &value)?;

        // Start building upper lanes
        while index != 0 && (index + 1) % LEVEL_BASE == 0 {
            let lane_value = lane.rev_base_iterator(index, LEVEL_BASE)?
                .map(|(_, val)| {
                    match val {
                        Ok(val) => val,
                        Err(err) => panic!("Skip list database failure: {}", err)
                    }
                })
                .fold(None, |state: Option<C>, value: C| {
                    if let Some(mut state) = state {
                        state.merge(&value);
                        Some(state)
                    } else {
                        Some(value)
                    }
                })
                .unwrap();

            value = lane_value;
            index = ((index + 1) / LEVEL_BASE) - 1;
            lane = lane.higher_lane();
            lane.put(index, &value)?;
        }

        self.state.levels = max(lane.level() + 1, self.state.levels);
        self.state.len += 1;

        self.list_db.put(&self.list_id, &self.state)
            .map_err(SkipListError::from)
    }


    /// Build a difference object between two indexes.
    /// `list.get(from).merge(list.diff(from, to)) == list.get(to)`
    /// !!! Experimental !!!
    fn diff(&self, from: usize, to: usize) -> Result<Option<C>, SkipListError> {
        if from > to || from >= self.state.len {
            return Ok(None);
        }

        let mut lane = Lane::new(self.list_id, 0, self.lane_db.clone());
        let mut pos = NodeHeader::new(self.list_id, lane.level(), from);
        let mut acc = Default::default();
        let mut ascend = true;

        loop {
            let mut merge = true;

            // Diff should be done in three phases:
            // Phase 1 is base crawling:
            // - You want to get to the "edge" to ascend to as higher lane as possible by crawling on lower lanes
            // - Skipping values whilst ascending, merging while crawling
            // Phase 2 is to take the fastest lane
            // Phase 3 natural descend
            if pos.base_index() == to && pos.level() == 0 {
                return Ok(Some(acc));
            } else {
                if ascend && pos.is_edge_node() && pos.higher().next().base_index() <= to {
                    pos = pos.higher();
                    lane = lane.higher_lane();
                    merge = false;
                } else if pos.next().base_index() > to {
                    pos = pos.lower();
                    lane = lane.lower_lane();
                    ascend = false;
                } else {
                    pos = pos.next()
                }

                if merge {
                    if let Some(value) = lane.get(pos.index())? {
                        acc.merge(&value)
                    } else {
                        panic!("Value not found in lanes, even thou it should: \
                                    current_level: {} | current_lane_index: {} | current_index: {}",
                               lane.level(), pos.index(), pos.base_index());
                    }
                }
            }
        }
    }
}

pub struct DatabaseBackedFlatList<C> {
    lane_db: Arc<LaneDatabase>,
    list_db: Arc<SkipListDatabase>,
    list_id: SkipListId,
    state: SkipListState,
    curr_context: C,
}

impl<C> KeyValueSchema for DatabaseBackedFlatList<C> {
    type Key = SkipListId;
    type Value = SkipListState;

    fn name() -> &'static str {
        "flat_skip_list"
    }
}

impl<C: Default> DatabaseBackedFlatList<C> {
    pub fn new(list_id: SkipListId, db: Arc<rocksdb::DB>) -> Result<Self, SkipListError> {
        let list_db: Arc<SkipListDatabase> = db.clone();
        let state = list_db.get(&list_id)?
            .unwrap_or_else(|| SkipListState {
                levels: 1,
                len: 0,
            });
        Ok(Self { lane_db: db, list_db, list_id, state, curr_context: Default::default() })
    }
}

impl<C> SkipList for DatabaseBackedFlatList<C> {
    /// Get number of elements stored in this node
    #[inline]
    fn len(&self) -> usize {
        self.state.len
    }

    #[inline]
    fn levels(&self) -> usize {
        1
    }

    /// Check, that given index is stored in structure
    #[inline]
    fn contains(&self, index: usize) -> bool {
        self.state.len > index
    }
}

impl<K: Codec, V: Codec, C: ListValue<K, V>> TypedSkipList<K, V, C> for DatabaseBackedFlatList<C> {
    fn get(&self, index: usize) -> Result<Option<C>, SkipListError> {
        if self.contains(index) {
            let lane = Lane::new(self.list_id, 0, self.lane_db.clone());
            lane.get(index)
        } else {
            Ok(None)
        }
    }

    fn get_key(&self, index: usize, key: &K) -> Result<Option<V>, SkipListError> {
        Ok(self.get(index)?.and_then(|c| c.get(key)))
    }

    fn push(&mut self, value: C) -> Result<(), SkipListError> {
        let lane = Lane::new(self.list_id, 0, self.lane_db.clone());
        let index = self.state.len;
        self.curr_context.merge(&value);

        lane.put(index, &self.curr_context)?;
        self.state.len += 1;

        self.list_db.put(&self.list_id, &self.state)
            .map_err(SkipListError::from)
    }

    fn diff(&self, _from: usize, _to: usize) -> Result<Option<C>, SkipListError> {
        unimplemented!()
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