use crate::{
    chunks::ChunkedVec,
    gc::SortedMap,
    persistent::file::{File, TAG_NEW_SHAPES},
    working_tree::string_interner::StringId,
};
use modular_bitfield::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

// /// Contains the shape (key fragments) of a directory.
// ///
// /// A `DirectoryShapeId` maps to a slice of `StringId`
// pub struct DirectoryShapes {
//     /// Map `DirectoryShapeHash` to its `DirectoryShapeId` and strings.
//     hash_to_strings: SortedMap<DirectoryShapeHash, (DirectoryShapeId, ShapeSliceId)>,
//     shapes: ChunkedVec<StringId, { 64 * 1024 * 1024 }>,

//     to_serialize: Vec<ShapeSliceId>,

//     /// Map the `DirectoryShapeId` to its `DirectoryShapeHash`.
//     id_to_hash: IndexMap<DirectoryShapeId, DirectoryShapeHash, { 100 * 1024 }>,
//     /// Temporary vector used to collect the `StringId` when creating/retrieving a shape.
//     temp: Vec<StringId>,
// }

struct ShapesFile {
    file: File<TAG_NEW_SHAPES>,
    in_mem: ChunkedVec<StringId, { 64 * 1024 * 1024 }>,
    first_index_in_mem: usize,
}

struct ShapesOnDisk {
    hash_to_strings: SortedMap<DirectoryShapeHash, (DirectoryShapeId, ShapeSliceId)>,
    shapes: ShapesFile,
}
