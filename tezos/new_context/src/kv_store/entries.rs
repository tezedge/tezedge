use std::{
    convert::{TryFrom, TryInto},
    marker::PhantomData,
};

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
    K: TryInto<usize>,
{
    pub fn set(&mut self, key: K, value: T) -> Result<(), K::Error> {
        self.entries[key.try_into()?] = value;
        Ok(())
    }

    pub fn get(&self, key: K) -> Result<Option<&T>, K::Error> {
        Ok(self.entries.get(key.try_into()?))
    }

    pub fn get_mut(&mut self, key: K) -> Result<Option<&mut T>, K::Error> {
        Ok(self.entries.get_mut(key.try_into()?))
    }
}

impl<K, V> Entries<K, V>
where
    K: TryInto<usize>,
    K: TryFrom<usize>,
    V: Default,
{
    pub fn get_vacant_entry(&mut self) -> Result<(K, &mut V), <K as TryFrom<usize>>::Error> {
        let current = self.entries.len();
        self.entries.push(Default::default());
        Ok((K::try_from(current)?, &mut self.entries[current]))
    }

    pub fn insert_at(&mut self, key: K, value: V) -> Result<(), <K as TryInto<usize>>::Error> {
        let index: usize = key.try_into()?;

        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, V::default);
        }

        self.entries[index] = value;
        Ok(())
    }
}
