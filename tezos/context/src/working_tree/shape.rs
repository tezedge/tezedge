// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{
        btree_map::Entry::{Occupied, Vacant},
        hash_map::DefaultHasher,
        BTreeMap,
    },
    convert::{TryFrom, TryInto},
    hash::Hasher,
};

use crate::kv_store::index_map::IndexMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{storage::DirEntryId, string_interner::StringId};

#[derive(Debug, Error)]
pub enum DirectoryShapeError {
    #[error("ShapeId not found")]
    ShapeIdNotFound,
    #[error("Cannot find key")]
    CannotFindKey,
    #[error("IdFromUSize")]
    IdFromUSize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DirectoryShapeId(u32);

impl TryInto<usize> for DirectoryShapeId {
    type Error = DirectoryShapeError;

    fn try_into(self) -> Result<usize, Self::Error> {
        Ok(self.0 as usize)
    }
}

impl TryFrom<usize> for DirectoryShapeId {
    type Error = DirectoryShapeError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        let value: u32 = value
            .try_into()
            .map_err(|_| DirectoryShapeError::IdFromUSize)?;
        Ok(Self(value))
    }
}

impl DirectoryShapeId {
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for DirectoryShapeId {
    fn from(shape_id: u32) -> Self {
        Self(shape_id)
    }
}

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct DirectoryShapeHash(u64);

/// Contains the shape (key fragments) of a directory.
///
/// A `DirectoryShapeId` maps to a slice of `StringId`
pub struct DirectoryShapes {
    /// Map `DirectoryShapeHash` to its `DirectoryShapeId` and strings.
    hash_to_strings: BTreeMap<DirectoryShapeHash, (DirectoryShapeId, Box<[StringId]>)>,
    /// Map the `DirectoryShapeId` to its `DirectoryShapeHash`.
    id_to_hash: IndexMap<DirectoryShapeId, DirectoryShapeHash>,
    /// Temporary vector used to collect the `StringId` when creating/retrieving a shape.
    temp: Vec<StringId>,
}

impl Default for DirectoryShapes {
    fn default() -> Self {
        Self::new()
    }
}

pub enum ShapeStrings<'a> {
    SliceIds(&'a [StringId]),
    Owned(Vec<String>),
}

impl DirectoryShapes {
    pub fn new() -> Self {
        Self {
            hash_to_strings: BTreeMap::default(),
            id_to_hash: IndexMap::with_capacity(1024),
            temp: Vec::with_capacity(256),
        }
    }

    pub fn nshapes(&self) -> usize {
        self.id_to_hash.len()
    }

    pub fn get_shape(
        &self,
        shape_id: DirectoryShapeId,
    ) -> Result<&[StringId], DirectoryShapeError> {
        let hash = match self.id_to_hash.get(shape_id)?.copied() {
            Some(hash) => hash,
            None => return Err(DirectoryShapeError::ShapeIdNotFound),
        };

        self.hash_to_strings
            .get(&hash)
            .map(|s| &*s.1)
            .ok_or(DirectoryShapeError::ShapeIdNotFound)
    }

    pub fn make_shape(
        &mut self,
        dir: &[(StringId, DirEntryId)],
    ) -> Result<Option<DirectoryShapeId>, DirectoryShapeError> {
        self.temp.clear();

        let mut hasher = DefaultHasher::new();
        hasher.write_usize(dir.len());

        for (key_id, _) in dir {
            if key_id.is_big() {
                return Ok(None);
            }

            hasher.write_u32(key_id.as_u32());
            self.temp.push(*key_id);
        }

        let shape_hash = DirectoryShapeHash(hasher.finish());

        match self.hash_to_strings.entry(shape_hash) {
            Occupied(entry) => Ok(Some(entry.get().0)),
            Vacant(entry) => {
                let shape_id = self.id_to_hash.push(shape_hash)?;
                entry.insert((shape_id, Box::from(self.temp.as_slice())));
                Ok(Some(shape_id))
            }
        }
    }
}
