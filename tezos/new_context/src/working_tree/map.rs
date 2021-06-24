use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, rc::Rc};

/// Immutable ordered map
#[derive(Clone, Serialize, Deserialize, Eq)]
pub struct Map<K, V>
where
    K: Ord,
{
    keys: Rc<[(K, V)]>,
}

impl<K, V> Default for Map<K, V>
where
    K: Ord,
{
    fn default() -> Self {
        Self { keys: Rc::new([]) }
    }
}

impl<K, V> Debug for Map<K, V>
where
    K: Ord + Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Map [\n")?;
        for (k, v) in &*self.keys {
            f.write_str(&format!(" ({:?}: {:?})\n", k, v))?;
        }
        f.write_str("]")
    }
}

impl<K, V> PartialEq for Map<K, V>
where
    K: Ord + PartialEq + Eq,
    V: PartialEq + Eq,
{
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter()
            .zip(other.iter())
            .find(|(a, b)| a != b)
            .is_none()
    }
}

impl<K, V> Map<K, V>
where
    K: Ord,
{
    pub fn new() -> Self {
        Map { keys: Rc::new([]) }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.len() == 0
    }

    pub fn iter(&self) -> MapIter<K, V> {
        MapIter {
            map: &self,
            current: 0,
        }
    }

    pub fn consuming_iter(self) -> ConsumingMapIter<K, V> {
        ConsumingMapIter {
            map: self,
            current: 0,
        }
    }

    pub fn values(&self) -> MapValues<K, V> {
        MapValues {
            map: &self,
            current: 0,
        }
    }
}

impl<K, V> Map<K, V>
where
    K: Ord,
    K: Clone,
    V: Clone,
{
    pub fn get<BK>(&self, key: &BK) -> Option<&V>
    where
        BK: Ord + ?Sized,
        K: Borrow<BK>,
    {
        let key = key.borrow();
        let index = self
            .keys
            .binary_search_by(|value| value.0.borrow().cmp(key))
            .ok()?;

        self.keys.get(index).map(|v| &v.1)
    }

    pub fn insert(&self, key: K, value: V) -> Self {
        let index = self.keys.binary_search_by(|value| value.0.cmp(&key));

        match index {
            Ok(found) => {
                let mut new_map = Self {
                    keys: (&*self.keys).into(),
                };
                let keys = Rc::get_mut(&mut new_map.keys).unwrap();
                keys[found].1 = value;

                new_map
            }
            Err(index) => {
                let mut new_keys = Vec::with_capacity(self.keys.len() + 1);
                new_keys.extend_from_slice(&self.keys[..index]);
                new_keys.push((key, value));
                new_keys.extend_from_slice(&self.keys[index..]);

                Self {
                    keys: new_keys.into(),
                }
            }
        }
    }

    pub fn remove<BK>(&self, key: &BK) -> Self
    where
        BK: Ord + ?Sized,
        K: Borrow<BK>,
    {
        let key = key.borrow();

        let index = match self
            .keys
            .binary_search_by(|value| value.0.borrow().cmp(&key))
        {
            Ok(index) => index,
            Err(_) => return self.clone(),
        };

        let mut new_keys = Vec::with_capacity(self.keys.len() - 1);

        if index > 0 {
            new_keys.extend_from_slice(&self.keys[..index]);
        }
        if index != self.keys.len() {
            new_keys.extend_from_slice(&self.keys[index + 1..]);
        }

        Self {
            keys: new_keys.into(),
        }
    }
}

impl<'a, K, V> IntoIterator for &'a Map<K, V>
where
    K: Ord,
    K: Clone,
    V: Clone,
{
    type Item = &'a (K, V);
    type IntoIter = MapIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K, V> IntoIterator for Map<K, V>
where
    K: Ord,
    K: Clone,
    V: Clone,
{
    type Item = (K, V);
    type IntoIter = ConsumingMapIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.consuming_iter()
    }
}

pub struct ConsumingMapIter<K, V>
where
    K: Ord,
{
    map: Map<K, V>,
    current: usize,
}

impl<K, V> Iterator for ConsumingMapIter<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.map.keys.get(self.current).cloned();
        self.current += 1;
        current
    }
}

pub struct MapValues<'a, K, V>
where
    K: Ord,
{
    map: &'a Map<K, V>,
    current: usize,
}

impl<'a, K, V> Iterator for MapValues<'a, K, V>
where
    K: Ord,
{
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.map.keys.get(self.current).map(|v| &v.1);
        self.current += 1;
        current
    }
}

pub struct MapIter<'a, K, V>
where
    K: Ord,
{
    map: &'a Map<K, V>,
    current: usize,
}

impl<'a, K, V> Iterator for MapIter<'a, K, V>
where
    K: Ord,
{
    type Item = &'a (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.map.keys.get(self.current);
        self.current += 1;
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_working_tree() {
        let map = Map::new();

        let m1 = map.insert("c", 1);
        let m2 = m1.insert("a", 2);
        let m3 = m2.insert("b", 3);
        let m4 = m3.insert("0", 4);
        let m5 = m4.insert("aa", 5);
        let m6 = m5.remove("c");

        assert_eq!(*m5.get(&"b").unwrap(), 3);

        assert_ne!(m1, m2);
        assert_eq!(m1, m1.clone());

        println!("MAP={:#?}", map);
        println!("M1={:#?}", m1);
        println!("M2={:#?}", m2);
        println!("M3={:#?}", m3);
        println!("M4={:#?}", m4);
        println!("M5={:#?}", m5);
        println!("M6={:#?}", m6);
    }
}
