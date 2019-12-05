// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::max;
use std::marker::PhantomData;
use std::sync::Arc;

use rocksdb::DB;

use storage::persistent::KeyValueSchema;

use crate::{
    content::{ListValue, NodeHeader},
    lane::Lane,
    LEVEL_BASE,
};

/// Data structure implementation, managing structure data, shape and metadata
/// It is expected, that structure will hold a few GiBs of data, and because of that,
/// structure is backed by an database.
pub struct SkipList<C: ListValue> {
    container: Arc<DB>,
    levels: usize,
    len: usize,
    _pd: PhantomData<C>,
}

impl<C: ListValue> KeyValueSchema for SkipList<C> {
    type Key = NodeHeader;
    type Value = C;

    fn name() -> &'static str {
        "skip_list"
    }
}

impl<C: ListValue> SkipList<C> {
    /// Create new list in given database
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            container: db,
            levels: 1,
            len: 0,
            _pd: PhantomData,
        }
    }

    /// Get number of elements stored in this node
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn levels(&self) -> usize {
        self.levels
    }

    /// Check, that given index is stored in structure
    pub fn contains(&self, index: usize) -> bool {
        self.len > index
    }

    /// Rebuild state for given index
    pub fn get(&self, index: usize) -> Option<C> {
        let mut current_state: Option<C> = None;
        let mut lane = Lane::new(Self::index_level(index), self.container.clone());
        let mut pos = NodeHeader::new(lane.level(), 0);

        // There is an sequential index on lowest level, if expected index is bigger than
        // length of chain, it is not stored, otherwise, IT MUST BE FOUND.
        if index >= self.len {
            return None;
        }

        loop {
            if let Some(value) = lane.get(pos.index()) {
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

            if pos.base_index() == index && lane.level() == 0 {
                return current_state;
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
    pub fn push(&mut self, mut value: C) {
        let mut lane = Lane::new(0, self.container.clone());
        let mut index = self.len;

        // Insert value into lowest level, as is.
        println!("inserting into bottom lane on index: {}", index);
        lane.put(index, &value);

        // Start building upper lanes
        while index != 0 && (index + 1) % LEVEL_BASE == 0 {
            let lane_value = lane.base_iterator(index).unwrap()
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
            lane.put(index, &value);
        }

        self.levels = max(lane.level() + 1, self.levels);
        self.len += 1;
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
