use crate::{
    lane::Lane,
    content::{ListValue, NodeHeader},
    LEVEL_BASE,
};
use std::sync::Arc;
use rocksdb::DB;
use std::marker::PhantomData;
use storage::persistent::Schema;

/// Data structure implementation, managing structure data, shape and metadata
/// It is expected, that structure will hold a few GiBs of data, and because of that,
/// structure is backed by an database.
pub struct SkipList<C: ListValue> {
    container: Arc<DB>,
    levels: usize,
    len: usize,
    _pd: PhantomData<C>,
}

impl<C: ListValue> Schema for SkipList<C> {
    const COLUMN_FAMILY_NAME: &'static str = "skip_list";
    type Key = NodeHeader;
    type Value = C;
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

    /// Check, that given index is stored in structure
    pub fn contains(&self, index: usize) -> bool {
        self.len > index
    }

    /// Rebuild state for given index
    pub fn get(&self, index: usize) -> Option<C> {
        let mut current_level = self.levels - 1;
        let mut current_state: Option<C> = None;
        let mut current_pos = 0;
        let mut current_index = 0;
        let mut lane: Lane<C> = Lane::new(self.levels, self.container.clone());

        loop {
            // TODO: Make level configurable
            let step = LEVEL_BASE.pow(current_level as u32);

            if let Some(value) = lane.get(current_index) {
                if let Some(mut state) = current_state {
                    state.merge(&value);
                    current_state = Some(state);
                } else {
                    current_state = Some(value);
                }
            } else {
                // List does not contains the requested value
                return None;
            }

            if current_pos + step > index {
                // We cannot move horizontally anymore, so we need to update state with descending data
                if current_level == 0 {
                    // We hit bottom and we cannot move horizontally. We are done.
                    return current_state;
                } else {
                    // Make a descend
                    current_level -= 1;
                    lane = lane.lower_lane();
                    current_level *= LEVEL_BASE;
                }
            } else {
                // We can still move horizontally on highest lane.
                current_index += 1;
            }

            current_pos += step;
        }
    }

    /// Push new value into the end of the list. Beware, this is operation is
    /// not thread safe and should be handled with care !!!
    pub fn push(&mut self, mut value: &C) {
        let mut current_level = 0;
        let mut lane = Lane::new(current_level, self.container.clone());
        let mut index = self.len;

        // Insert value into lowest level, unchanged.
        lane.put(index, value);

        // Start building upper lanes
        while index != 0 && (index + 1) % LEVEL_BASE == 0 {
            let lane_value = lane.base_iterator(index).unwrap()
                .take(LEVEL_BASE)
                .map(|(_, val)| {
                    match val {
                        Ok(val) => val,
                        Err(err) => val.expect(&format!("Skip list database failure: {}", err))
                    }
                })
                .fold(None, |mut state, value| {
                    if let Some(mut state) = state {
                        state.diff(value);
                    } else {
                        state = Some(value);
                    }
                }).unwrap();
            index = (index + 1 / LEVEL_BASE) - 1;
            current_level += 1;
            lane = lane.higher_lane();
            lane.put(index, lane_value);
        }

        self.len += 1;
    }
}