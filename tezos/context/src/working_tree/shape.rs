// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    borrow::Cow,
    collections::hash_map::DefaultHasher,
    convert::{TryFrom, TryInto},
    hash::Hasher,
    io::Read,
};

use crate::{
    chunks::ChunkedVec,
    gc::{Entry, SortedMap},
    kv_store::index_map::IndexMap,
    persistent::file::{File, TAG_SHAPE, TAG_SHAPE_INDEX},
    serialize::DeserializationError,
};
use modular_bitfield::prelude::*;
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
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

#[bitfield]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub struct ShapeSliceId {
    start: B47,
    length: B17,
}

/// Contains the shape (key fragments) of a directory.
///
/// A `DirectoryShapeId` maps to a slice of `StringId`
pub struct DirectoryShapes {
    /// Map `DirectoryShapeHash` to its `DirectoryShapeId` and strings.
    hash_to_strings: SortedMap<DirectoryShapeHash, (DirectoryShapeId, ShapeSliceId)>,
    shapes: ChunkedVec<StringId, { 64 * 1024 * 1024 }>,

    to_serialize: Vec<ShapeSliceId>,

    /// Map the `DirectoryShapeId` to its `DirectoryShapeHash`.
    id_to_hash: IndexMap<DirectoryShapeId, DirectoryShapeHash, { 100 * 1024 }>,
    /// Temporary vector used to collect the `StringId` when creating/retrieving a shape.
    temp: Vec<StringId>,
}

impl std::fmt::Debug for DirectoryShapes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectoryShapes")
            .field("hash_to_strings_bytes", &self.hash_to_strings.total_bytes())
            .field("shapes_cap", &self.shapes.capacity())
            .field(
                "shapes_bytes",
                &(self.shapes.capacity() * std::mem::size_of::<StringId>()),
            )
            .field("to_serialize_cap", &self.to_serialize.capacity())
            .field(
                "to_serialize_bytes",
                &(self.to_serialize.capacity() * std::mem::size_of::<ShapeSliceId>()),
            )
            .field("id_to_hash_cap", &self.id_to_hash.capacity())
            .field(
                "id_to_hash_bytes",
                &(self.id_to_hash.capacity() * std::mem::size_of::<DirectoryShapeHash>()),
            )
            .field("temp_cap", &self.temp.capacity())
            .finish()
    }
}

#[derive(Debug, Default)]
pub struct SerializeShape {
    pub shapes: Vec<u8>,
    pub index: Vec<u8>,
}

impl Default for DirectoryShapes {
    fn default() -> Self {
        Self::new()
    }
}

pub enum ShapeStrings<'a> {
    SliceIds(Cow<'a, [StringId]>),
    Owned(Vec<String>),
}

impl<'a> ShapeStrings<'a> {
    pub fn len(&self) -> usize {
        match self {
            ShapeStrings::SliceIds(slice) => slice.len(),
            ShapeStrings::Owned(owned) => owned.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl DirectoryShapes {
    pub fn new() -> Self {
        Self {
            hash_to_strings: Default::default(),
            id_to_hash: IndexMap::default(),
            temp: Vec::with_capacity(256),
            shapes: ChunkedVec::default(), // 64 MB
            to_serialize: Vec::with_capacity(256),
        }
    }

    pub fn nshapes(&self) -> usize {
        self.id_to_hash.len()
    }

    pub fn get_shape(
        &self,
        shape_id: DirectoryShapeId,
    ) -> Result<Cow<[StringId]>, DirectoryShapeError> {
        let hash = match self.id_to_hash.get(shape_id)?.copied() {
            Some(hash) => hash,
            None => return Err(DirectoryShapeError::ShapeIdNotFound),
        };

        let slice_id = self
            .hash_to_strings
            .get(&hash)
            .map(|s| s.1)
            .ok_or(DirectoryShapeError::ShapeIdNotFound)?;

        let start: usize = slice_id.start() as usize;
        let end: usize = start + slice_id.length() as usize;

        self.shapes
            .get_slice(start..end)
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
            hasher.write_u32(key_id.as_u32());
            self.temp.push(*key_id);
        }

        let shape_hash = DirectoryShapeHash(hasher.finish());

        match self.hash_to_strings.entry(shape_hash) {
            Entry::Occupied(entry) => Ok(Some(entry.0)),
            Entry::Vacant(entry) => {
                let start = self.shapes.len() as u64;
                let length = self.temp.len() as u32;

                self.shapes.extend_from_slice(self.temp.as_slice());

                let slice_id = ShapeSliceId::new().with_start(start).with_length(length);

                self.to_serialize.push(slice_id);

                let shape_id = self.id_to_hash.push(shape_hash)?;
                entry.insert((shape_id, slice_id));
                Ok(Some(shape_id))
            }
        }
    }

    pub fn serialize(&mut self) -> SerializeShape {
        let mut output = SerializeShape::default();

        for slice_id in &self.to_serialize {
            let start: usize = slice_id.start() as usize;
            let end: usize = start + slice_id.length() as usize;

            let shape = self.shapes.get_slice(start..end).unwrap();

            for string_id in shape.as_ref() {
                let string_id: u32 = string_id.as_u32();
                output.shapes.extend_from_slice(&string_id.to_le_bytes());
            }

            let slice_bytes: [u8; 8] = slice_id.into_bytes();
            output.index.extend_from_slice(&slice_bytes);
        }

        self.to_serialize.clear();

        output
    }

    pub fn deallocate_serialized(&mut self) {
        let first: usize = match self.to_serialize.get(0) {
            Some(first) => first.start() as usize,
            None => return,
        };

        self.shapes.deallocate_before(first);
    }

    pub fn deserialize(
        shapes_file: File<{ TAG_SHAPE }>,
        shapes_index_file: File<{ TAG_SHAPE_INDEX }>,
    ) -> Result<Self, DeserializationError> {
        let mut result = Self::default();
        let mut string_id_bytes = [0u8; 4];

        let mut offset = shapes_file.start();
        let shape_file_end = shapes_file.offset().as_u64();

        let mut shapes_file = shapes_file.buffered()?;

        while offset < shape_file_end {
            shapes_file.read_exact(&mut string_id_bytes)?;

            let string_id = StringId::deserialize(string_id_bytes);
            offset += string_id_bytes.len() as u64;

            result.shapes.push(string_id);
        }

        let mut offset = shapes_index_file.start();
        let shape_index_file_end = shapes_index_file.offset().as_u64();
        let mut shape_slice_id_bytes = [0u8; 8];

        let mut shapes_index_file = shapes_index_file.buffered()?;

        while offset < shape_index_file_end {
            shapes_index_file.read_exact(&mut shape_slice_id_bytes)?;

            offset += shape_slice_id_bytes.len() as u64;

            let slice_id: ShapeSliceId = ShapeSliceId::from_bytes(shape_slice_id_bytes);

            let start: usize = slice_id.start() as usize;
            let end: usize = start + slice_id.length() as usize;

            let slice = result.shapes.get_slice(start..end).unwrap();

            result.temp.clear();
            let mut hasher = DefaultHasher::new();
            hasher.write_usize(slice.len());
            for key_id in slice.as_ref() {
                hasher.write_u32(key_id.as_u32());
                result.temp.push(*key_id);
            }
            let shape_hash = DirectoryShapeHash(hasher.finish());

            let shape_id = result.id_to_hash.push(shape_hash)?;
            result
                .hash_to_strings
                .insert(shape_hash, (shape_id, slice_id));
        }

        Ok(result)
    }

    pub fn total_bytes(&self) -> usize {
        let hash_to_strings = self.hash_to_strings.total_bytes();
        let shapes = self.shapes.capacity() * std::mem::size_of::<StringId>();
        let id_to_hash = self.id_to_hash.capacity() * std::mem::size_of::<DirectoryShapeHash>();
        let to_serialize = self.to_serialize.capacity() * std::mem::size_of::<ShapeSliceId>();
        let temp = self.temp.capacity() * std::mem::size_of::<StringId>();

        hash_to_strings + shapes + id_to_hash + to_serialize + temp
    }

    pub fn shrink_to_fit(&mut self) {
        self.hash_to_strings.shrink_to_fit();
        self.to_serialize = Vec::with_capacity(256);
    }
}
