// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![allow(clippy::identity_op)]

//! This sub module provides different implementations of the `repository` used to store objects.

use std::convert::{TryFrom, TryInto};

use modular_bitfield::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    chunks::{ChunkedVec, SharedIndexMap},
    gc::worker::NEW_IDS_CHUNK_CAPACITY,
    ObjectHash,
};

use self::in_memory::OBJECTS_CHUNK_CAPACITY;

pub mod hashes;
pub mod in_memory;
pub mod index_map;
pub mod inline_boxed_slice;
pub mod persistent;
pub mod readonly_ipc;

pub const INMEM: &str = "inmem";

#[allow(dead_code)]
#[bitfield]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct NonZero6BytesInner {
    bytes: B48,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct NonZero6Bytes {
    inner: NonZero6BytesInner,
}

impl NonZero6Bytes {
    fn get(self) -> u64 {
        self.inner.bytes()
    }

    fn new(n: u64) -> Option<Self> {
        if n == 0 || n > 0xFFFFFFFFFFFF {
            None
        } else {
            Some(NonZero6Bytes {
                inner: NonZero6BytesInner::new().with_bytes(n),
            })
        }
    }

    // This avoid having compilation warnings
    #[allow(dead_code)]
    fn unused(self) {
        NonZero6BytesInner::from_bytes(self.inner.into_bytes());
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HashId(NonZero6Bytes);

impl std::fmt::Debug for HashId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hash_id = self.as_u64();
        f.debug_tuple("HashId").field(&hash_id).finish()
    }
}

#[derive(Debug, Error)]
pub struct HashIdError;

impl std::fmt::Display for HashIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashIdError").finish()
    }
}

impl TryInto<usize> for HashId {
    type Error = HashIdError;

    fn try_into(self) -> Result<usize, Self::Error> {
        Ok(self.0.get().checked_sub(1).ok_or(HashIdError)? as usize)
    }
}

impl TryFrom<usize> for HashId {
    type Error = HashIdError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        let value: u64 = value.try_into().map_err(|_| HashIdError)?;

        value
            .checked_add(1)
            .and_then(NonZero6Bytes::new)
            .map(HashId)
            .ok_or(HashIdError)
    }
}

/// `HashId` is a `NonZeroU64` (8 bytes), but in the working tree and repo,
/// they are 6 bytes. So the shift must be 47, or we will get overflow if
/// shift is more than 47.
/// See `DirEntryInner::object_hash_id`, `PointerToInodeInner::hash_id`, and
/// `serialize::serialize_hash_id`.
const SHIFT: usize = 48 - 1;
/// Bit set when the HashId hasn't been commited
const IN_WORKING_TREE: u64 = 1 << SHIFT;

impl HashId {
    pub fn new(value: u64) -> Option<Self> {
        Some(HashId(NonZero6Bytes::new(value)?))
    }

    pub fn as_u64(&self) -> u64 {
        self.0.get()
    }

    pub fn set_in_working_tree(&mut self) -> Result<(), HashIdError> {
        let hash_id = self.0.get();

        self.0 = NonZero6Bytes::new(hash_id | IN_WORKING_TREE).ok_or(HashIdError)?;

        Ok(())
    }

    pub fn get_in_working_tree(self) -> Result<Option<HashId>, HashIdError> {
        let hash_id = self.0.get();
        if hash_id & IN_WORKING_TREE != 0 {
            Ok(Some(HashId(
                NonZero6Bytes::new(hash_id & !IN_WORKING_TREE).ok_or(HashIdError)?,
            )))
        } else {
            Ok(None)
        }
    }
}

enum Vacant<'a> {
    ByRef {
        entry_ref: &'a mut ObjectHash,
        hash_id: HashId,
    },
    Push {
        map: &'a mut SharedIndexMap<HashId, Option<Box<ObjectHash>>, OBJECTS_CHUNK_CAPACITY>,
        new_ids: &'a mut ChunkedVec<HashId, { NEW_IDS_CHUNK_CAPACITY }>,
    },
    UseFreeId {
        map: &'a mut SharedIndexMap<HashId, Option<Box<ObjectHash>>, OBJECTS_CHUNK_CAPACITY>,
        hash_id: HashId,
    },
}

pub struct VacantObjectHash<'a> {
    vacant: Vacant<'a>,
    is_working_tree: bool,
}

impl<'a> VacantObjectHash<'a> {
    pub fn new(entry_ref: &'a mut ObjectHash, hash_id: HashId) -> Self {
        Self {
            vacant: Vacant::ByRef { entry_ref, hash_id },
            is_working_tree: false,
        }
    }

    pub fn new_existing_id(
        map: &'a mut SharedIndexMap<HashId, Option<Box<ObjectHash>>, OBJECTS_CHUNK_CAPACITY>,
        hash_id: HashId,
    ) -> Self {
        Self {
            vacant: Vacant::UseFreeId { map, hash_id },
            is_working_tree: false,
        }
    }

    pub fn new_push(
        map: &'a mut SharedIndexMap<HashId, Option<Box<ObjectHash>>, OBJECTS_CHUNK_CAPACITY>,
        new_ids: &'a mut ChunkedVec<HashId, NEW_IDS_CHUNK_CAPACITY>,
    ) -> Self {
        Self {
            vacant: Vacant::Push { map, new_ids },
            is_working_tree: false,
        }
    }

    pub fn write_with<F>(self, fun: F) -> Result<HashId, HashIdError>
    where
        F: FnOnce(&mut ObjectHash),
    {
        let mut hash_id = match self.vacant {
            Vacant::ByRef { entry_ref, hash_id } => {
                fun(entry_ref);
                hash_id
            }
            Vacant::Push { map, new_ids } => {
                let mut hash = Box::default();

                fun(&mut hash);

                let hash_id = map.push(hash)?;
                new_ids.push(hash_id);

                hash_id
            }
            Vacant::UseFreeId { map, hash_id } => {
                let mut hash = Box::default();
                fun(&mut hash);

                map.insert_at(hash_id, hash)?;

                hash_id
            }
        };

        if self.is_working_tree {
            hash_id.set_in_working_tree()?;
        }

        Ok(hash_id)
    }

    pub(crate) fn set_readonly_runner(mut self) -> Self {
        self.is_working_tree = true;
        self
    }
}
