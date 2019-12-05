// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::max;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use storage::persistent::{BincodeEncoded, DatabaseWithSchema, KeyValueSchema};

use crate::{content::{ListValue, NodeHeader}, lane::Lane, LEVEL_BASE, SkipListError};
use crate::content::SkipListId;
use crate::lane::LaneDatabase;

pub type SkipListDatabase<C> = dyn DatabaseWithSchema<SkipList<C>> + Sync + Send;

/// Data structure implementation, managing structure data, shape and metadata
/// It is expected, that structure will hold a few GiBs of data, and because of that,
/// structure is backed by an database.
pub struct SkipList<C: ListValue> {
    lane_db: Arc<LaneDatabase<C>>,
    list_db: Arc<SkipListDatabase<C>>,
    list_id: SkipListId,
    state: SkipListState,
}

impl<C: ListValue> KeyValueSchema for SkipList<C> {
    type Key = SkipListId;
    type Value = SkipListState;

    fn name() -> &'static str {
        "skip_list"
    }
}

impl<C: ListValue> SkipList<C> {
    /// Create new list in given database
    pub fn new(list_id: SkipListId, db: Arc<rocksdb::DB>) -> Result<Self, SkipListError> {
        let list_db: Arc<SkipListDatabase<C>> = db.clone();
        let state = list_db.get(&list_id)?
            .unwrap_or_else(|| SkipListState {
                levels: 1,
                len: 0,
            });

        Ok(Self { lane_db: db, list_db, list_id, state })
    }

    /// Get number of elements stored in this node
    #[inline]
    pub fn len(&self) -> usize {
        self.state.len
    }

    #[inline]
    pub fn levels(&self) -> usize {
        self.state.levels
    }

    /// Check, that given index is stored in structure
    #[inline]
    pub fn contains(&self, index: usize) -> bool {
        self.state.len > index
    }

    /// Rebuild state for given index
    pub fn get(&self, index: usize) -> Result<Option<C>, SkipListError> {
        let mut current_state: Option<C> = None;
        let mut lane = Lane::new(self.list_id, Self::index_level(index), self.lane_db.clone());
        let mut pos = NodeHeader::new(lane.level(), 0);

        // There is an sequential index on lowest level, if expected index is bigger than
        // length of chain, it is not stored, otherwise, IT MUST BE FOUND.
        if index >= self.state.len {
            return Ok(None);
        }

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

            if (pos.base_index() == index) && (lane.level() == 0) {
                return Ok(current_state);
            } else if pos.next().base_index() > index {
                // We cannot move horizontally anymore, so we need to descent to lower level.
                if lane.level() == 0 {
                    // We hit bottom and we cannot move horizontally. Yet it is not requested index
                    // Something gone wrong.
                    panic!("Correct value was skipped");
                } else {
                    // Make a descend
                    lane = lane.lower_lane();
                    pos = pos.lower();
                }
            } else {
                // We can still move horizontally on current lane.
                pos = pos.next();
            }
        }
    }

    /// Push new value into the end of the list. Beware, this is operation is
    /// not thread safe and should be handled with care !!!
    pub fn push(&mut self, mut value: C) -> Result<(), SkipListError> {
        let mut lane = Lane::new(self.list_id, 0, self.lane_db.clone());
        let mut index = self.state.len;

        // Insert value into lowest level, as is.
        lane.put(index, &value)?;

        // Start building upper lanes
        while index != 0 && (index + 1) % LEVEL_BASE == 0 {
            let lane_value = lane.base_iterator(index)?
                .take(LEVEL_BASE)
                .map(|(_, val)| {
                    match val {
                        Ok(val) => val,
                        Err(err) => panic!("Skip list database failure: {}", err)
                    }
                })
                .fold(None, |state: Option<C>, value| {
                    if let Some(mut state) = state {
                        state.diff(&value);
                        Some(state)
                    } else {
                        Some(value)
                    }
                }).unwrap();
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

    /// Find highest level, which we should traverse to hit the index
    pub fn index_level(index: usize) -> usize {
        if index == 0 {
            0
        } else {
            f64::log((index + 1) as f64, LEVEL_BASE as f64).floor() as usize
        }
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