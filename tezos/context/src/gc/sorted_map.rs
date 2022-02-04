use std::fmt::Debug;

#[cfg(not(test))]
const MAX_CHUNK_SIZE: usize = 1_000_000;

#[cfg(test)]
const MAX_CHUNK_SIZE: usize = 4;

const SPLIT_AT: usize = (MAX_CHUNK_SIZE / 2) + (MAX_CHUNK_SIZE / 4);

struct Chunk<K, V>
where
    K: Ord,
{
    inner: Vec<(K, V)>,
    min: K,
    max: K,
}

pub struct SortedMap<K, V>
where
    K: Ord,
{
    list: Vec<Chunk<K, V>>,
}

impl<K, V> Default for SortedMap<K, V>
where
    K: Ord,
{
    fn default() -> Self {
        Self {
            list: Default::default(),
        }
    }
}

impl<K, V> std::fmt::Debug for Chunk<K, V>
where
    K: Ord + std::fmt::Debug,
    V: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chunk")
            .field("inner", &self.inner)
            .field("inner_cap", &self.inner.capacity())
            .field("min", &self.min)
            .field("max", &self.max)
            .finish()
    }
}

impl<K, V> std::fmt::Debug for SortedMap<K, V>
where
    K: Ord + std::fmt::Debug,
    V: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut len = 0;
        let mut cap = 0;

        for chunk in &self.list {
            len += chunk.inner.len();
            cap += chunk.inner.capacity();
        }

        f.debug_struct("SortedMap")
            .field("nchunks", &self.list.len())
            .field("list_len", &len)
            .field("list_cap", &cap)
            .finish()

        // f.debug_struct("SortedMap")
        //     .field("list", &self.list)
        //     .finish()
    }
}

impl<K, V> Chunk<K, V>
where
    K: Ord + Copy,
{
    fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit();
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn new_with_element(key: K, value: V) -> Self {
        let mut inner = Vec::with_capacity(MAX_CHUNK_SIZE / 2);

        let min = key;
        let max = key;

        inner.push((key, value));

        Self { inner, min, max }
    }

    fn set_min_max(&mut self) {
        if let Some(first) = self.inner.first() {
            self.min = first.0;
        };
        if let Some(last) = self.inner.last() {
            self.max = last.0;
        };
    }

    fn get(&self, key: &K) -> Option<&V> {
        let index = self.inner.binary_search_by_key(key, |&(k, _)| k).ok()?;
        self.inner.get(index).map(|(_, v)| v)
    }

    fn append(&mut self, other: &mut Self) {
        self.inner.append(&mut other.inner);
        self.max = self.inner.last().unwrap().0;
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        let index = self.inner.binary_search_by_key(key, |&(k, _)| k).ok()?;
        let item = self.inner.remove(index);

        Some(item.1)
    }

    fn insert(&mut self, key: K, value: V) -> Option<Self> {
        let insert_at = match self.inner.binary_search_by_key(&key, |&(k, _)| k) {
            Ok(index) => {
                self.inner[index].1 = value;
                return None;
            }
            Err(index) => index,
        };

        let mut next_chunk = None;

        if self.len() == MAX_CHUNK_SIZE {
            let new_chunk = if insert_at >= SPLIT_AT {
                let mut new_chunk = self.inner.split_off(SPLIT_AT);
                new_chunk.insert(insert_at - SPLIT_AT, (key, value));
                new_chunk
            } else {
                let new_chunk = self.inner.split_off(SPLIT_AT - 1);
                self.inner.insert(insert_at, (key, value));
                new_chunk
            };

            let new_min = new_chunk[0].0;
            let new_max = new_chunk.last().unwrap().0;

            self.max = self.inner.last().unwrap().0;

            next_chunk = Some(Chunk {
                inner: new_chunk,
                min: new_min,
                max: new_max,
            });
        } else {
            self.inner.insert(insert_at, (key, value));

            if key < self.min {
                self.min = key;
            }

            if key > self.max {
                self.max = key;
            }
        }

        next_chunk
    }
}

impl<K, V> SortedMap<K, V>
where
    // K: Ord + Copy,
    K: Ord + Copy + Debug,
{
    pub fn shrink_to_fit(&mut self) {
        for chunk in self.list.iter_mut() {
            chunk.shrink_to_fit();
        }
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    pub fn keys_to_vec(mut self) -> Vec<K> {
        let mut vec = Vec::with_capacity(self.len());

        while let Some(chunk) = self.list.get(0) {
            for (key, _) in &chunk.inner {
                vec.push(*key);
            }
            self.list.remove(0);
        }

        vec
    }

    pub fn len(&self) -> usize {
        self.list.iter().fold(0, |acc, c| acc + c.len())
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let index = self.binary_search(key).ok()?;
        let chunk = self.list.get(index)?;

        chunk.get(key)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let index = self.binary_search(key).ok()?;

        let chunk = &mut self.list[index];
        let item = chunk.remove(key);

        if chunk.is_empty() {
            self.list.remove(index);
        } else {
            chunk.set_min_max();
        }

        item
    }

    pub fn insert(&mut self, key: K, value: V) {
        match self.binary_search(&key) {
            Ok(index) => {
                let chunk = &mut self.list[index];

                let mut new_chunk = match chunk.insert(key, value) {
                    Some(new_chunk) => new_chunk,
                    None => return,
                };

                let can_merge_with_next = self
                    .list
                    .get(index + 1)
                    .map(|c| c.len() + new_chunk.len() < MAX_CHUNK_SIZE)
                    .unwrap_or(false);

                if can_merge_with_next {
                    new_chunk.append(self.list.get_mut(index + 1).unwrap());
                    self.list[index + 1] = new_chunk
                } else {
                    self.list.insert(index + 1, new_chunk);
                }
            }
            Err(index) => {
                if let Some(prev) = index.checked_sub(1) {
                    let next_size = self.list.get(index).map(|c| c.len());

                    let prev_chunk = &mut self.list[prev];

                    let next_is_less_busy = next_size
                        .map(|next_size| next_size < prev_chunk.len())
                        .unwrap_or(false);

                    if !next_is_less_busy && prev_chunk.len() < MAX_CHUNK_SIZE {
                        let new = prev_chunk.insert(key, value);
                        debug_assert!(new.is_none());
                        return;
                    }
                };

                if self.list.len() == index {
                    self.list.push(Chunk::new_with_element(key, value));
                    return;
                }

                let chunk = &mut self.list[index];

                if chunk.len() < MAX_CHUNK_SIZE {
                    let new = chunk.insert(key, value);
                    debug_assert!(new.is_none());
                    return;
                }

                let chunk = Chunk::new_with_element(key, value);
                self.list.insert(index, chunk);
            }
        }
    }

    fn binary_search(&self, key: &K) -> Result<usize, usize> {
        let key = *key;

        self.list.binary_search_by(|chunk| {
            if chunk.min > key {
                std::cmp::Ordering::Greater
            } else if chunk.max < key {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
    }

    #[cfg(test)]
    fn assert_correct(&self) {
        let mut prev_chunk = Option::<(K, K)>::None;

        for chunk in &self.list {
            if let Some(prev_chunk) = prev_chunk {
                assert!(prev_chunk.0 < chunk.min);
                assert!(prev_chunk.1 < chunk.max);
            };

            assert!(chunk.min <= chunk.max);
            assert_eq!(chunk.min, chunk.inner[0].0);
            assert_eq!(chunk.max, chunk.inner.last().unwrap().0);

            let mut prev = None;
            for item in &chunk.inner {
                if let Some(prev) = prev {
                    assert!(prev < item.0);
                }
                prev = Some(item.0);
            }

            prev_chunk = Some((chunk.min, chunk.max));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sorted_map() {
        let mut map = SortedMap::default();

        map.insert(101, 101);
        map.insert(110, 110);
        map.insert(100, 100);
        map.insert(100, 100);
        map.insert(102, 102);

        assert_eq!(map.list.len(), 1);

        map.insert(104, 104);
        assert_eq!(map.list.len(), 2);

        map.insert(114, 114);
        map.insert(200, 200);
        assert_eq!(map.list.len(), 2);
        map.insert(201, 201);
        assert_eq!(map.list.len(), 3);
        map.insert(120, 120);
        map.insert(119, 119);
        map.insert(115, 115);
        map.insert(116, 116);

        map.insert(117, 117);
        map.insert(117, 1117);

        assert_eq!(map.list.len(), 4);

        map.insert(99, 99);
        assert_eq!(map.list.len(), 4);
        map.insert(98, 98);
        assert_eq!(map.list.len(), 5);

        println!("MAP={:#?}", map);

        map.assert_correct();

        map.remove(&200);
        map.remove(&120);
        assert_eq!(map.list.len(), 5);
        map.assert_correct();
        map.remove(&201);
        assert_eq!(map.list.len(), 4);

        map.remove(&98);
        map.assert_correct();
        assert_eq!(map.list.len(), 3);

        map.remove(&104);
        map.remove(&110);
        map.assert_correct();
        assert_eq!(map.list.len(), 3);
        map.remove(&114);
        assert_eq!(map.list.len(), 2);

        println!("MAP={:#?}", map);

        map.assert_correct();
    }

    #[test]
    fn test_sorted_map_big() {
        let mut map = SortedMap::default();

        for index in 0..100_000 {
            map.insert(index, index);
        }
        map.assert_correct();

        assert_eq!(map.list.len(), 25_000);
    }

    #[test]
    fn test_sorted_map_big_revert() {
        let mut map = SortedMap::default();

        for index in 0..100_000 {
            map.insert(100_000 - index, index);
        }
        map.assert_correct();

        assert_eq!(map.list.len(), 25_000);
    }
}
