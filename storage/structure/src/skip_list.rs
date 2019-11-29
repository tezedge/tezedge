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
    fn index_level(index: usize) -> usize {
        if index == 0 {
            0
        } else {
            f64::log((index + 1) as f64, LEVEL_BASE as f64).floor() as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common_testing::*;

    use super::*;

    #[test]
    pub fn list_new() {
        let tmp = TmpDb::new();
        let list = SkipList::<Value>::new(tmp.db());
        assert_eq!(list.len(), 0);
    }

    #[test]
    pub fn list_push() {
        let tmp = TmpDb::new();
        let mut list = SkipList::<Value>::new(tmp.db());
        list.push(Value::new(vec![1]));
        assert!(list.contains(0));
    }

    #[test]
    pub fn list_index_level() {
        type List = SkipList<Value>;
        for index in 0..7 {
            assert_eq!(List::index_level(index), 0);
        }
        assert_eq!(List::index_level(7), 1);
        assert_eq!(List::index_level(16), 1);
        assert_eq!(List::index_level(552), 3);
    }

    #[test]
    pub fn list_check_first() {
        let tmp = TmpDb::new();
        let mut list = SkipList::<Value>::new(tmp.db());
        list.push(Value::new(vec![1]));
        let val = list.get(0);
        assert_eq!(val.is_some(), list.contains(0), "List `get` and `contains` return inconsistent answers");
        assert!(val.is_some());
        assert_eq!(val.unwrap(), Value::new(vec![1]));
    }

    #[test]
    pub fn list_check_second() {
        let tmp = TmpDb::new();
        let mut list = SkipList::<Value>::new(tmp.db());
        list.push(Value::new(vec![1]));
        list.push(Value::new(vec![2]));
        let val = list.get(1);
        assert_eq!(val.is_some(), list.contains(1), "List `get` and `contains` return inconsistent answers");
        assert!(val.is_some());
        assert_eq!(val.unwrap(), Value::new(vec![1, 2]));
    }

    #[test]
    pub fn list_check_bottom_lane() {
        let tmp = TmpDb::new();
        let mut list = SkipList::<Value>::new(tmp.db());
        for index in 0..=6 {
            list.push(Value::new(vec![index]));
        }
        assert_eq!(list.levels(), 1);
        let val = list.get(6);
        assert_eq!(val.is_some(), list.contains(6), "List `get` and `contains` return inconsistent answers");
        assert!(val.is_some());
        assert_eq!(val.unwrap(), Value::new((0..=6).collect()));
    }

    #[test]
    pub fn list_check_faster_lane() {
        let tmp = TmpDb::new();
        let mut list = SkipList::<Value>::new(tmp.db());
        for index in 0..=7 {
            list.push(Value::new(vec![index]));
        }
        assert_eq!(list.levels(), 2);
        let val = list.get(7);
        assert_eq!(val.is_some(), list.contains(7), "List `get` and `contains` return inconsistent answers");
        assert!(val.is_some());
        assert_eq!(val.unwrap(), Value::new((0..=7).collect()));
    }

    #[test]
    pub fn list_check_lane_traversal() {
        let tmp = TmpDb::new();
        let mut list = SkipList::<Value>::new(tmp.db());
        for index in 0..=63 {
            list.push(Value::new(vec![index]));
        }
        assert_eq!(list.levels(), 3);
        let val = list.get(63);
        assert_eq!(val.is_some(), list.contains(63), "List `get` and `contains` return inconsistent answers");
        assert!(val.is_some());
        assert_eq!(val.unwrap(), Value::new((0..=63).collect()));
    }

    #[test]
    pub fn list_check_lane_order_traversal() {
        let tmp = TmpDb::new();
        let mut list = SkipList::<OrderedValue>::new(tmp.db());
        for (value, key) in (0..=63).zip((0..=7).cycle()) {
            let mut map = HashMap::new();
            map.insert(key, value);
            list.push(OrderedValue::new(map));
        }
        assert_eq!(list.levels(), 3);
        let val = list.get(63);
        assert_eq!(val.is_some(), list.contains(63), "List `get` and `contains` return inconsistent answers");
        assert!(val.is_some());
        let mut expected = HashMap::new();
        for (value, key) in (56..=63).zip(0..=7) {
            expected.insert(key, value);
        }
        assert_eq!(val.unwrap(), OrderedValue::new(expected));
    }
}