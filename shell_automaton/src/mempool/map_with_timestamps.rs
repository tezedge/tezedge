// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{vec_deque, BTreeMap, VecDeque};
use std::hash::Hash;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BTreeMapWithTimestamps<T>
where
    T: Hash + Ord,
{
    map: BTreeMap<T, u64>,
    order: VecDeque<(T, u64)>,
}

impl<T> Default for BTreeMapWithTimestamps<T>
where
    T: Hash + Ord,
{
    fn default() -> Self {
        Self {
            map: Default::default(),
            order: Default::default(),
        }
    }
}

impl<T> BTreeMapWithTimestamps<T>
where
    T: Hash + Ord + Clone,
{
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn insert(&mut self, key: T, timestamp: u64) -> Option<u64> {
        let res = if let Some(t) = self.map.insert(key.clone(), timestamp) {
            self.remove_order(&key, t);
            Some(t)
        } else {
            None
        };
        let i = self.order.partition_point(|(_, t)| *t <= timestamp);
        self.order.insert(i, (key, timestamp));
        res
    }

    pub fn contains_key(&self, key: &T) -> bool {
        self.map.contains_key(key)
    }

    pub fn remove(&mut self, key: &T) -> Option<u64> {
        if let Some(timestamp) = self.map.remove(key) {
            self.remove_order(key, timestamp);
            Some(timestamp)
        } else {
            None
        }
    }

    pub fn extend_with_timestamp<I>(&mut self, timestamp: u64, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        let iter = iter.into_iter();
        self.order.reserve(iter.size_hint().0);
        let i = self.order.partition_point(|(_, t)| *t <= timestamp);
        let drain = self.order.drain(i..).collect::<Vec<_>>();
        for v in iter {
            self.map.insert(v.clone(), timestamp);
            self.order.push_back((v, timestamp));
        }
        self.order.extend(drain.into_iter());
    }

    pub fn earliest_timestamp(&self) -> Option<u64> {
        self.order.front().map(|&(_, t)| t)
    }

    pub fn range_older_than(&self, timestamp: u64) -> impl Iterator<Item = &(T, u64)> {
        let partition = self.order.partition_point(|&(_, t)| t < timestamp);
        self.order.range(..partition)
    }

    pub fn drain_older_than(&mut self, timestamp: u64) -> BTreeMapWithTimestampsDrain<'_, T> {
        let partition = self.order.partition_point(|&(_, t)| t < timestamp);
        let map = &mut self.map;
        let drain = self.order.drain(..partition);
        BTreeMapWithTimestampsDrain { map, drain }
    }

    fn remove_order(&mut self, key: &T, timestamp: u64) {
        let mut i = self.order.partition_point(|(_, t)| *t < timestamp);
        while i < self.order.len() && self.order[i].1 == timestamp {
            if self.order[i].0 == *key {
                self.order.remove(i);
                break;
            }
            i += 1;
        }
    }
}

impl<T, const N: usize> From<[(T, u64); N]> for BTreeMapWithTimestamps<T>
where
    T: Hash + Ord + Clone,
{
    fn from(mut source: [(T, u64); N]) -> Self {
        source.sort_by_key(|&(_, t)| t);
        let map = BTreeMap::from(source.clone());
        let order = VecDeque::from(source);
        Self { map, order }
    }
}

pub struct BTreeMapWithTimestampsDrain<'a, T>
where
    T: Hash + Ord,
{
    map: &'a mut BTreeMap<T, u64>,
    drain: vec_deque::Drain<'a, (T, u64)>,
}

impl<'a, T> Iterator for BTreeMapWithTimestampsDrain<'a, T>
where
    T: Hash + Ord,
{
    type Item = (T, u64);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((v, t)) = self.drain.next() {
            self.map.remove(&v);
            Some((v, t))
        } else {
            None
        }
    }
}

impl<'a, T> Drop for BTreeMapWithTimestampsDrain<'a, T>
where
    T: Hash + Ord,
{
    fn drop(&mut self) {
        let Self { map, drain } = self;
        drain.for_each(|(v, _)| {
            map.remove(&v);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::BTreeMapWithTimestamps;

    #[test]
    fn contains_key() {
        let map = BTreeMapWithTimestamps::from([
            ("a", 1),
            ("b", 1),
            ("c", 1),
            ("d", 3),
            ("e", 5),
            ("f", 9),
        ]);

        assert!(map.contains_key(&"a"));
        assert!(map.contains_key(&"c"));
        assert!(map.contains_key(&"f"));
        assert!(!map.contains_key(&"z"));
    }

    #[test]
    fn insert() {
        let mut map = BTreeMapWithTimestamps::new();
        map.insert("a", 1);
        map.insert("b", 1);
        map.insert("c", 1);

        assert_eq!(
            map,
            BTreeMapWithTimestamps::from([("a", 1), ("b", 1), ("c", 1)])
        );

        assert_eq!(map.insert("a", 10), Some(1));
        assert_eq!(map.insert("b", 0), Some(1));
        assert_eq!(map.insert("c", 6), Some(1));
        assert_eq!(map.insert("e", 8), None);

        assert_eq!(
            map,
            BTreeMapWithTimestamps::from([("b", 0), ("c", 6), ("e", 8), ("a", 10)])
        );
    }

    #[test]
    fn remove() {
        let mut map = BTreeMapWithTimestamps::from([
            ("a", 1),
            ("b", 1),
            ("c", 1),
            ("d", 3),
            ("e", 5),
            ("f", 9),
        ]);

        assert_eq!(map.remove(&"a"), Some(1));
        assert_eq!(map.remove(&"d"), Some(3));
        assert_eq!(map.remove(&"c"), Some(1));
        assert_eq!(map.remove(&"z"), None);

        assert_eq!(
            map,
            BTreeMapWithTimestamps::from([("b", 1), ("e", 5), ("f", 9)])
        );
    }

    #[test]
    fn extend_with_timestamp() {
        let mut map = BTreeMapWithTimestamps::from([("d", 3), ("e", 5), ("f", 9)]);
        map.extend_with_timestamp(1, ["a", "b", "c"]);
        assert_eq!(
            map,
            BTreeMapWithTimestamps::from([
                ("a", 1),
                ("b", 1),
                ("c", 1),
                ("d", 3),
                ("e", 5),
                ("f", 9),
            ])
        );

        let mut map = BTreeMapWithTimestamps::from([("d", 3), ("e", 5), ("f", 9)]);
        map.extend_with_timestamp(4, ["a", "b", "c"]);
        assert_eq!(
            map,
            BTreeMapWithTimestamps::from([
                ("d", 3),
                ("a", 4),
                ("b", 4),
                ("c", 4),
                ("e", 5),
                ("f", 9),
            ])
        );

        let mut map = BTreeMapWithTimestamps::from([("d", 3), ("e", 5), ("f", 9)]);
        map.extend_with_timestamp(10, ["a", "b", "c"]);
        assert_eq!(
            map,
            BTreeMapWithTimestamps::from([
                ("d", 3),
                ("e", 5),
                ("f", 9),
                ("a", 10),
                ("b", 10),
                ("c", 10),
            ])
        );
    }

    #[test]
    fn drain() {
        let map = BTreeMapWithTimestamps::from([
            ("a", 1),
            ("b", 1),
            ("c", 1),
            ("d", 3),
            ("e", 5),
            ("f", 9),
        ]);

        let mut drop_map = map.clone();
        drop_map.drain_older_than(2);
        assert_eq!(
            drop_map,
            BTreeMapWithTimestamps::from([("d", 3), ("e", 5), ("f", 9)])
        );

        let mut drain_map = map.clone();
        let drain = drain_map.drain_older_than(2).collect::<Vec<_>>();
        assert_eq!(
            drain_map,
            BTreeMapWithTimestamps::from([("d", 3), ("e", 5), ("f", 9)])
        );
        assert_eq!(drain, vec![("a", 1), ("b", 1), ("c", 1)]);
    }
}
