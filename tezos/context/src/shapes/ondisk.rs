use std::{borrow::Cow, collections::hash_map::DefaultHasher, hash::Hasher, io::Read};

use crate::{
    chunks::ChunkedVec,
    gc::{Entry, SortedMap},
    persistent::file::{File, TAG_NEW_SHAPES, TAG_NEW_SHAPES_INDEX},
    working_tree::{storage::DirEntryId, string_interner::StringId},
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

pub struct ShapesFile {
    pub file: File<TAG_NEW_SHAPES>,
    in_mem: ChunkedVec<StringId, { 64 * 1024 }>,
    first_index_in_mem: usize,
}

pub struct ShapesIndexFile {
    pub file: File<TAG_NEW_SHAPES_INDEX>,
    in_mem: ChunkedVec<ShapeSliceId, { 64 * 1024 }>,
    first_index_in_mem: usize,
}

pub struct ShapesOnDisk {
    hash_to_strings: SortedMap<DirectoryShapeHash, DirectoryShapeId>,
    pub shapes: ShapesFile,
    pub shapes_index: ShapesIndexFile,
    /// Temporary vector used to collect the `StringId` when creating/retrieving a shape.
    temp: Vec<StringId>,
}

impl std::fmt::Debug for ShapesOnDisk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShapesOnDisk")
            .field("hash_to_strings_len", &self.hash_to_strings.len())
            .field("hash_to_strings_bytes", &self.hash_to_strings.total_bytes())
            .field("shapes_bytes", &self.shapes.total_bytes())
            .field("shapes_index_bytes", &self.shapes_index.total_bytes())
            .field("temp_cap", &self.temp.capacity())
            .field("total_bytes", &self.total_bytes())
            .finish()
    }
}

impl ShapesOnDisk {
    pub fn new(
        shapes_file: File<TAG_NEW_SHAPES>,
        shapes_index_file: File<TAG_NEW_SHAPES_INDEX>,
    ) -> Self {
        Self {
            hash_to_strings: Default::default(),
            temp: Vec::with_capacity(256),
            shapes: ShapesFile::new(shapes_file),
            shapes_index: ShapesIndexFile::new(shapes_index_file),
        }
    }

    pub fn reload(mut self) -> std::io::Result<Self> {
        let mut offset = self.shapes_index.file.start();
        let shape_index_file_end = self.shapes_index.file.offset().as_u64();

        if offset == shape_index_file_end {
            return Ok(self);
        }

        let mut shape_slice_id_bytes = [0u8; 8];

        let mut shapes_index_file = self.shapes_index.file.buffered()?;

        let mut temp = Vec::with_capacity(256);
        let mut index: usize = 0;

        // eprintln!("OFFSET={:?} END={:?}", offset, shape_index_file_end);

        while offset < shape_index_file_end {
            // eprintln!("INDEX={:?}", index);

            shapes_index_file.read_exact(&mut shape_slice_id_bytes)?;

            offset += shape_slice_id_bytes.len() as u64;

            let slice_id: ShapeSliceId = ShapeSliceId::from_bytes(shape_slice_id_bytes);

            // eprintln!("SLICE_ID={:?}", slice_id);

            // let start: usize = slice_id.start() as usize;
            // let end: usize = start + slice_id.length() as usize;

            // TODO: Bufferize the shape file ?
            let slice = self.shapes.get_shape(slice_id).unwrap();
            // let slice = result.shapes.get_slice(start..end).unwrap();

            temp.clear();
            let mut hasher = DefaultHasher::new();
            hasher.write_usize(slice.len());
            for key_id in slice.as_ref() {
                hasher.write_u32(key_id.as_u32());
                temp.push(*key_id);
            }
            let shape_hash = DirectoryShapeHash(hasher.finish());

            let shape_id = DirectoryShapeId::try_from(index).unwrap();

            // let shape_id = self.id_to_hash.push(shape_hash)?;
            self.hash_to_strings.insert(shape_hash, shape_id);

            index += 1;
        }

        eprintln!("shape reloaded {:#?}", self);
        self.hash_to_strings.shrink_to_fit();
        eprintln!("shape reloaded after shrink {:#?}", self);

        // self.shapes_index.file.file = shapes_index_file.into_inner();

        Ok(self)
    }

    pub fn shrink_to_fit(&mut self) {
        self.hash_to_strings.shrink_to_fit();
    }

    pub fn write_to_disk(&mut self) -> std::io::Result<()> {
        self.shapes.write_to_disk()?;
        self.shapes_index.write_to_disk()?;

        Ok(())
    }

    pub fn sync(&mut self) -> std::io::Result<()> {
        self.shapes.sync()?;
        self.shapes_index.sync()?;

        Ok(())
    }

    pub fn total_bytes(&self) -> usize {
        self.hash_to_strings.total_bytes()
            + self.shapes.total_bytes()
            + self.shapes_index.total_bytes()
    }

    pub fn nshapes(&self) -> usize {
        self.hash_to_strings.len()
    }

    pub fn get_shape(
        &self,
        shape_id: DirectoryShapeId,
    ) -> Result<Cow<[StringId]>, DirectoryShapeError> {
        // eprintln!("GET_SHAPE={:?}", shape_id);
        let shape_slice_id = self.shapes_index.get_shape_slice(shape_id)?;
        let res = self.shapes.get_shape(shape_slice_id);
        // eprintln!("GET_SHAPE {:?}={:?} shape_slice_id={:?}", shape_id, res, shape_slice_id);
        res
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
            Entry::Occupied(entry) => Ok(Some(*entry)),
            Entry::Vacant(entry) => {
                let start = self.shapes.len() as u64;
                let length = self.temp.len() as u32;

                self.shapes.extend_from_slice(self.temp.as_slice());

                let slice_id = ShapeSliceId::new().with_start(start).with_length(length);
                let shape_id = self.shapes_index.add_shape_slice(slice_id)?;

                entry.insert(shape_id);

                // eprintln!("NEW SHAPE {:?} DIR={:?} IN_MEM={:?} SHAPE_SLICE_ID={:?}", shape_id, dir, self.shapes.in_mem, slice_id);
                // eprintln!("NEW SHAPE {:?} LEN={:?} TMP_LEN={:?} DIR={:?}", shape_id, dir.len(), length, dir);

                Ok(Some(shape_id))
            }
        }
    }
}

impl ShapesFile {
    pub fn new(file: File<TAG_NEW_SHAPES>) -> Self {
        let first_index_in_mem =
            (file.offset().as_u64() - file.start()) as usize / std::mem::size_of::<StringId>();

        Self {
            file,
            in_mem: ChunkedVec::default(),
            first_index_in_mem,
        }
    }

    fn total_bytes(&self) -> usize {
        self.in_mem.total_bytes()
    }

    fn write_to_disk(&mut self) -> std::io::Result<()> {
        let mut output = Vec::with_capacity(64 * 1024);

        for string_id in self.in_mem.iter() {
            let string_id = string_id.as_u32();
            output.extend_from_slice(&string_id.to_le_bytes());
        }

        self.file.append(output)?;
        self.first_index_in_mem += self.in_mem.len();
        self.in_mem.clear(); // TODO: Deallocate

        Ok(())
    }

    fn sync(&mut self) -> std::io::Result<()> {
        self.file.sync()
    }

    fn len(&self) -> usize {
        self.first_index_in_mem + self.in_mem.len()
    }

    fn extend_from_slice(&mut self, slice: &[StringId]) {
        self.in_mem.extend_from_slice(slice);
    }

    fn get_shape(
        &self,
        shape_slice_id: ShapeSliceId,
    ) -> Result<Cow<[StringId]>, DirectoryShapeError> {
        let offset: u64 = shape_slice_id.start();
        let length: u32 = shape_slice_id.length();

        let offset = offset as usize;

        // eprintln!("GET_SHAPE={:?} OFFSET={:?} FIRST={:?} IN_MEM={:?}", shape_slice_id, offset, self.first_index_in_mem, self.in_mem.len());

        if offset >= self.first_index_in_mem {
            let offset = offset - self.first_index_in_mem;
            let length = length as usize;
            let slice = self.in_mem.get_slice(offset..offset + length).unwrap();

            // eprintln!("SLICE={:?}", slice);

            Ok(slice)
        } else {
            const BUFFER: [u8; 256 * std::mem::size_of::<StringId>()] =
                [0; 256 * std::mem::size_of::<StringId>()];

            let mut buffer = BUFFER;
            let offset = offset as u64 * std::mem::size_of::<StringId>() as u64;
            let length = length as usize;

            let buffer = &mut buffer[0..length * std::mem::size_of::<StringId>()];

            // eprintln!("READ shape_slice_id={:?} offset={:?} length={:?} buf_len={:?}", shape_slice_id, offset, length, buffer.len());

            let offset = offset + self.file.start();
            self.file.read_exact_at(buffer, offset.into()).unwrap();

            let mut slice = Vec::with_capacity(length);

            for bytes in buffer.chunks_exact(std::mem::size_of::<StringId>()) {
                let bytes: [u8; 4] = bytes.try_into().unwrap();
                let string_id = StringId::deserialize(bytes);
                slice.push(string_id);
            }

            // eprintln!("LAAA {:?} {:?}", slice.len(), slice);

            Ok(Cow::Owned(slice))
        }
    }
}

impl ShapesIndexFile {
    pub fn new(file: File<TAG_NEW_SHAPES_INDEX>) -> Self {
        let first_index_in_mem = (file.offset().as_u64() - file.start()) as usize / 8;

        Self {
            file,
            in_mem: ChunkedVec::default(),
            first_index_in_mem,
        }
    }

    fn total_bytes(&self) -> usize {
        self.in_mem.total_bytes()
    }

    // pub fn serialize(&mut self) -> SerializeShape {
    //     let mut output = SerializeShape::default();

    //     for slice_id in &self.to_serialize {
    //         let start: usize = slice_id.start() as usize;
    //         let end: usize = start + slice_id.length() as usize;

    //         let shape = self.shapes.get_slice(start..end).unwrap();

    //         for string_id in shape.as_ref() {
    //             let string_id: u32 = string_id.as_u32();
    //             output.shapes.extend_from_slice(&string_id.to_le_bytes());
    //         }

    //         let slice_bytes: [u8; 8] = slice_id.into_bytes();
    //         output.index.extend_from_slice(&slice_bytes);
    //     }

    //     self.to_serialize.clear();

    //     output
    // }

    fn write_to_disk(&mut self) -> std::io::Result<()> {
        let mut output = Vec::with_capacity(64 * 1024);

        for shape_slice_id in self.in_mem.iter() {
            let shape_slice_id: [u8; 8] = shape_slice_id.into_bytes();

            output.extend_from_slice(&shape_slice_id[..]);
        }

        self.file.append(output)?;
        self.first_index_in_mem += self.in_mem.len();
        self.in_mem.clear(); // TODO: Deallocate

        Ok(())
    }

    fn sync(&mut self) -> std::io::Result<()> {
        self.file.sync()
    }

    fn add_shape_slice(
        &mut self,
        shape_slice_id: ShapeSliceId,
    ) -> Result<DirectoryShapeId, DirectoryShapeError> {
        let index = self.in_mem.push(shape_slice_id);
        let id = DirectoryShapeId::try_from(self.first_index_in_mem + index).unwrap();

        Ok(id)
    }

    fn get_shape_slice(
        &self,
        shape_id: DirectoryShapeId,
    ) -> Result<ShapeSliceId, DirectoryShapeError> {
        // eprintln!("get_shape_slice={:?} first={:?}", shape_id, self.first_index_in_mem);

        let shape_id: usize = shape_id.try_into()?;

        if shape_id >= self.first_index_in_mem {
            let shape_id = shape_id - self.first_index_in_mem;
            let shape_slice_id = self.in_mem.get(shape_id).copied().expect("Wrong index");
            Ok(shape_slice_id)
        } else {
            let mut buffer = <[u8; 8]>::default();
            let offset: u64 = shape_id as u64 * 8;

            let offset = offset + self.file.start();
            self.file.read_exact_at(&mut buffer, offset.into()).unwrap();
            Ok(ShapeSliceId::from_bytes(buffer))
        }
    }
}
