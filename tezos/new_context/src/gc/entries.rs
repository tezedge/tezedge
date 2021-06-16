use std::marker::PhantomData;

#[derive(Debug)]
pub struct Entries<K, V> {
    entries: Vec<V>,
    _phantom: PhantomData<K>,
}

impl<K, V> Entries<K, V> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

impl<K, T> Entries<K, T>
where
    K: Into<usize>,
{
    pub fn get(&self, key: K) -> Option<&T> {
        self.entries.get(key.into())
    }
}

impl<K, V> Entries<K, V>
where
    K: Into<usize>,
    K: From<usize>,
    V: Default,
{
    pub fn get_vacant_entry(&mut self) -> (K, &mut V) {
        let current = self.entries.len();
        self.entries.push(Default::default());
        (K::from(current), &mut self.entries[current])
    }

    pub fn insert_at(&mut self, key: K, value: V) {
        let index: usize = key.into();

        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, V::default);
        }

        self.entries[index] = value;
    }
}

impl<K, V> std::ops::Index<K> for Entries<K, V>
where
    K: Into<usize>,
{
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        &self.entries[index.into()]
    }
}

impl<K, V> std::ops::IndexMut<K> for Entries<K, V>
where
    K: Into<usize>,
{
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        &mut self.entries[index.into()]
    }
}
