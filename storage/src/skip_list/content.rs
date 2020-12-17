// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use failure::Fail;
use failure::_core::marker::PhantomData;
use rocksdb::{Cache, ColumnFamilyDescriptor, SliceTransform};
use serde::{Deserialize, Serialize};

use crate::num_from_slice;
use crate::persistent::database::IteratorWithSchema;
use crate::persistent::sequence::SequenceError;
use crate::persistent::{
    default_table_options, BincodeEncoded, Codec, DBError, Decoder, Encoder, KeyValueSchema,
    KeyValueStoreWithSchema, SchemaError,
};
use crate::skip_list::{TryExtend, LEVEL_BASE};

/// Structure for orientation in the list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeHeader {
    /// Skip list ID
    list_id: SkipListId,
    /// Level on which this node exists
    lane_level: usize,
    /// Position of node in lane
    node_index: usize,
}

impl NodeHeader {
    pub fn new(list_id: SkipListId, lane_level: usize, node_index: usize) -> Self {
        Self {
            list_id,
            lane_level,
            node_index,
        }
    }

    /// Get next node on this lane
    pub fn next(&self) -> Self {
        Self {
            list_id: self.list_id,
            lane_level: self.lane_level,
            node_index: self.node_index + 1,
        }
    }

    /// Get previous node on this lane, if node is first, do nothing
    pub fn prev(&self) -> Self {
        if self.node_index == 0 {
            self.clone()
        } else {
            Self {
                list_id: self.list_id,
                lane_level: self.lane_level,
                node_index: self.node_index - 1,
            }
        }
    }

    /// Move to the lower lane adapting the node index
    pub fn lower(&self) -> Self {
        if self.lane_level == 0 {
            self.clone()
        } else {
            Self {
                list_id: self.list_id,
                lane_level: self.lane_level - 1,
                node_index: self.lower_index(),
            }
        }
    }

    /// Move to the higher lane adapting the node index
    pub fn higher(&self) -> Self {
        Self {
            list_id: self.list_id,
            lane_level: self.lane_level + 1,
            node_index: self.higher_index(),
        }
    }

    /// Get index representing the current index from base(0.) lane
    pub fn base_index(&self) -> usize {
        if self.lane_level == 0 {
            self.node_index
        } else {
            ((self.node_index + 1) * LEVEL_BASE.pow(self.lane_level as u32)) - 1
        }
    }

    /// Calculate the index of the node on the lower lane
    pub fn lower_index(&self) -> usize {
        if self.lane_level == 0 {
            self.node_index
        } else {
            ((self.node_index + 1) * LEVEL_BASE) - 1
        }
    }

    /// Calculate the index of the node on the higher lane
    pub fn higher_index(&self) -> usize {
        if self.node_index < LEVEL_BASE {
            0
        } else {
            ((self.node_index + 1) / 8) - 1
        }
    }

    pub fn is_edge_node(&self) -> bool {
        (self.node_index + 1) % LEVEL_BASE == 0
    }

    pub fn level(&self) -> usize {
        self.lane_level
    }

    pub fn index(&self) -> usize {
        self.node_index
    }
}

impl BincodeEncoded for NodeHeader {}

pub type ListValueDatabase = dyn KeyValueStoreWithSchema<ListValue> + Sync + Send;

pub struct ListValue {
    id: usize,
    db: Arc<ListValueDatabase>,
}

impl ListValue {
    pub fn new(id: usize, db: Arc<ListValueDatabase>) -> Self {
        Self { id, db }
    }

    /// Merge two values into one, in-place
    pub fn merge(&mut self, other: &Self) -> Result<(), SkipListError> {
        for (key, value) in self.db.prefix_iterator(&ListValueKey::from_id(other.id))? {
            self.db
                .put(&ListValueKey::new(self.id, &key?.key), &value?)?;
        }

        Ok(())
    }
}

impl KeyValueSchema for ListValue {
    type Key = ListValueKey;
    type Value = Vec<u8>;

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = default_table_options(cache);
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(ListValueKey::LEN_ID));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "skip_list_values"
    }
}

impl<'a, K, V> TryExtend<(&'a K, &'a V)> for ListValue
where
    K: Encoder + 'a,
    V: Encoder + 'a,
{
    fn try_extend<T: IntoIterator<Item = (&'a K, &'a V)>>(
        &mut self,
        iter: T,
    ) -> Result<(), SkipListError> {
        for (key, value) in iter {
            self.db.put(
                &ListValueKey::new(self.id, &key.encode()?),
                &value.encode()?,
            )?;
        }
        Ok(())
    }
}

pub(crate) trait TypedListValue<'a, K, V> {
    fn get(&self, key: &K) -> Result<Option<V>, SkipListError>;

    fn iter(&'a self) -> Result<Iter<'a, K, V>, SkipListError>;

    fn iter_prefix(&'a self, prefix: &'a K) -> Result<IterPrefix<'a, K, V>, SkipListError>;
}

impl<'a, K: Codec, V: Decoder> TypedListValue<'a, K, V> for ListValue {
    /// Get value from stored container
    fn get(&self, value: &K) -> Result<Option<V>, SkipListError> {
        self.db
            .get(&ListValueKey::new(self.id, &value.encode()?))?
            .map(|bytes| V::decode(&bytes))
            .transpose()
            .map_err(SkipListError::from)
    }

    fn iter(&'a self) -> Result<Iter<'a, K, V>, SkipListError> {
        Iter::create(self.id, &self.db)
    }

    fn iter_prefix(&'a self, prefix: &'a K) -> Result<IterPrefix<'a, K, V>, SkipListError> {
        IterPrefix::create(self.id, prefix, &self.db)
    }
}

pub struct Iter<'a, K, V> {
    inner: IteratorWithSchema<'a, ListValue>,
    _phantom: PhantomData<(K, V)>,
}

impl<'a, K, V> Iter<'a, K, V> {
    fn create(id: usize, db: &'a Arc<ListValueDatabase>) -> Result<Self, SkipListError> {
        let inner = db.prefix_iterator(&ListValueKey::from_id(id))?;
        Ok(Self {
            inner,
            _phantom: PhantomData,
        })
    }
}

impl<'a, K: Decoder, V: Decoder> Iterator for Iter<'a, K, V> {
    type Item = Result<(K, V), SkipListError>;

    fn next(&mut self) -> Option<Self::Item> {
        extract_and_decode(self.inner.next())
    }
}

pub struct IterPrefix<'a, K, V> {
    inner: IteratorWithSchema<'a, ListValue>,
    prefix: Vec<u8>,
    _phantom: PhantomData<(K, V)>,
}

impl<'a, K: Encoder, V> IterPrefix<'a, K, V> {
    fn create(
        id: usize,
        prefix: &'a K,
        db: &'a Arc<ListValueDatabase>,
    ) -> Result<Self, SkipListError> {
        let prefix = prefix.encode()?;
        let inner = db.prefix_iterator(&ListValueKey::new(id, &prefix))?;
        Ok(Self {
            inner,
            prefix,
            _phantom: PhantomData,
        })
    }
}

impl<'a, K: Decoder, V: Decoder> Iterator for IterPrefix<'a, K, V> {
    type Item = Result<(K, V), SkipListError>;

    fn next(&mut self) -> Option<Self::Item> {
        let next: Option<(
            Result<ListValueKey, SchemaError>,
            Result<Vec<u8>, SchemaError>,
        )> = self.inner.next();
        if let Some((Ok(ListValueKey { key, .. }), _)) = &next {
            if !key.starts_with(&self.prefix) {
                return None;
            }
        }

        extract_and_decode(next)
    }
}

fn extract_and_decode<K: Decoder, V: Decoder>(
    value: Option<(
        Result<ListValueKey, SchemaError>,
        Result<Vec<u8>, SchemaError>,
    )>,
) -> Option<Result<(K, V), SkipListError>> {
    value.map(|(k, v)| {
        k.map(|k| k.key)
            .and_then(|key| K::decode(&key))
            .and_then(|k| v.and_then(|value| V::decode(&value)).map(|v| (k, v)))
            .map_err(SkipListError::from)
    })
}

trait StartsWith<T> {
    fn starts_with(&self, needle: &[T]) -> bool;
}

impl<T> StartsWith<T> for Vec<T>
where
    T: Sized + Eq,
{
    fn starts_with(&self, needle: &[T]) -> bool {
        if needle.len() > self.len() {
            return false;
        }

        for idx in (0..needle.len()).rev() {
            if self[idx] != needle[idx] {
                return false;
            }
        }

        true
    }
}

pub struct ListValueKey {
    id: usize,
    key: Vec<u8>,
}

impl ListValueKey {
    const LEN_ID: usize = std::mem::size_of::<usize>();
    const IDX_ID: usize = 0;
    const IDX_KEY: usize = Self::IDX_ID + Self::LEN_ID;

    fn new(id: usize, key: &[u8]) -> Self {
        Self {
            id,
            key: key.to_vec(),
        }
    }

    fn from_id(id: usize) -> Self {
        Self { id, key: vec![] }
    }
}

impl Decoder for ListValueKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() >= Self::LEN_ID {
            let id = num_from_slice!(bytes, Self::IDX_ID, usize);
            let key = bytes[Self::IDX_KEY..].to_vec();
            Ok(Self { id, key })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

impl Encoder for ListValueKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut bytes = Vec::with_capacity(Self::LEN_ID + self.key.len());
        bytes.extend(&self.id.to_be_bytes());
        bytes.extend(&self.key);
        Ok(bytes)
    }
}

/// ID of the skip list
pub type SkipListId = u16;

#[derive(Debug, Fail)]
pub enum SkipListError {
    #[fail(display = "Persistent storage error: {}", error)]
    PersistentStorageError { error: DBError },
    #[fail(
        display = "Internal error occurred during skip list operation: {}",
        description
    )]
    InternalError { description: String },
    #[fail(display = "Index is out of skip list bounds")]
    OutOfBoundsError,
    #[fail(display = "List value sequence error: {}", error)]
    SequenceError { error: SequenceError },
}

impl From<DBError> for SkipListError {
    fn from(error: DBError) -> Self {
        SkipListError::PersistentStorageError { error }
    }
}

impl From<SchemaError> for SkipListError {
    fn from(error: SchemaError) -> Self {
        SkipListError::PersistentStorageError {
            error: error.into(),
        }
    }
}

impl From<SequenceError> for SkipListError {
    fn from(error: SequenceError) -> Self {
        SkipListError::SequenceError { error }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub enum Bucket<V> {
    Exists(V),
    Deleted,
}

impl<V: Serialize + for<'a> Deserialize<'a>> BincodeEncoded for Bucket<V> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn header_next() {
        let original = NodeHeader::new(0, 0, 0);
        let next = original.next();
        assert_eq!(original.lane_level, next.lane_level);
        assert_eq!(original.node_index + 1, next.node_index);
    }

    #[test]
    pub fn header_level() {
        let original = NodeHeader::new(0, 0, 0);
        assert_eq!(original.level(), 0);
    }

    #[test]
    pub fn header_index() {
        let original = NodeHeader::new(0, 0, 0);
        assert_eq!(original.index(), 0);
    }
}
