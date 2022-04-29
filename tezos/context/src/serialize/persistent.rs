// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Serialization/deserialization for objects in the Working Tree so that they can be
//! saved/loaded to/from the repository.

use std::{borrow::Cow, convert::TryInto};

use modular_bitfield::prelude::*;
use serde::{Deserialize, Serialize};
use static_assertions::assert_eq_size;
use tezos_timing::SerializeStats;

use crate::{
    chunks::ChunkedVec,
    kv_store::{in_memory::BATCH_CHUNK_CAPACITY, inline_boxed_slice::InlinedBoxedSlice, HashId},
    serialize::{deserialize_hash_id, serialize_hash_id, ObjectTag},
    working_tree::{
        shape::ShapeStrings,
        storage::{
            DirectoryId, DirectoryOrInodeId, FatPointer, Inode, PointerOnStack, PointersBitfield,
            PointersId,
        },
        string_interner::StringInterner,
        working_tree::SerializeOutput,
        Commit, DirEntryKind, ObjectReference,
    },
    ContextKeyValueStore,
};

use crate::working_tree::{
    shape::DirectoryShapeId,
    storage::{DirEntryId, Storage},
    string_interner::StringId,
    DirEntry, Object,
};

use super::{DeserializationError, ObjectHeader, ObjectLength, SerializationError};

#[bitfield(bits = 8)]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub struct DirEntryShapeHeader {
    kind: DirEntryKind,
    blob_inline_length: B3,
    offset_length: RelativeOffsetLength,
    #[skip]
    _unused: B2,
}

// Must fit in 1 byte
assert_eq_size!(DirEntryShapeHeader, u8);

#[bitfield(bits = 8)]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub struct DirEntryHeader {
    kind: DirEntryKind,
    blob_inline_length: B3,
    key_inline_length: B2,
    offset_length: RelativeOffsetLength,
}

// Must fit in 1 byte
assert_eq_size!(DirEntryHeader, u8);

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
enum RelativeOffsetLength {
    OneByte,
    TwoBytes,
    FourBytes,
    EightBytes,
}

#[bitfield(bits = 8)]
struct CommitHeader {
    parent_offset_length: RelativeOffsetLength,
    root_offset_length: RelativeOffsetLength,
    author_length: ObjectLength,
    is_parent_exist: bool,
    #[skip]
    _unused: B1,
}

// Must fit in 1 byte
assert_eq_size!(CommitHeader, u8);

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct AbsoluteOffset(u64);
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct RelativeOffset(u64);

impl From<u64> for AbsoluteOffset {
    fn from(offset: u64) -> Self {
        Self(offset)
    }
}

impl From<u64> for RelativeOffset {
    fn from(offset: u64) -> Self {
        Self(offset)
    }
}

impl AbsoluteOffset {
    pub fn as_u64(self) -> u64 {
        self.0
    }
    pub fn add_offset(self, add: u64) -> Self {
        Self(self.0 + add)
    }
}

impl RelativeOffset {
    fn as_u64(self) -> u64 {
        self.0
    }
}

fn get_relative_offset(
    current_offset: AbsoluteOffset,
    target_offset: AbsoluteOffset,
) -> (RelativeOffset, RelativeOffsetLength) {
    let current_offset = current_offset.as_u64();
    let target_offset = target_offset.as_u64();

    assert!(current_offset >= target_offset);

    let relative_offset = current_offset - target_offset;

    if relative_offset <= 0xFF {
        (
            RelativeOffset(relative_offset),
            RelativeOffsetLength::OneByte,
        )
    } else if relative_offset <= 0xFFFF {
        (
            RelativeOffset(relative_offset),
            RelativeOffsetLength::TwoBytes,
        )
    } else if relative_offset <= 0xFFFFFFFF {
        (
            RelativeOffset(relative_offset),
            RelativeOffsetLength::FourBytes,
        )
    } else {
        (
            RelativeOffset(relative_offset),
            RelativeOffsetLength::EightBytes,
        )
    }
}

fn serialize_offset(
    output: &mut SerializeOutput,
    relative_offset: RelativeOffset,
    offset_length: RelativeOffsetLength,
    stats: &mut SerializeStats,
) -> Result<(), SerializationError> {
    let relative_offset = relative_offset.as_u64();

    match offset_length {
        RelativeOffsetLength::OneByte => {
            let offset: u8 = relative_offset as u8;
            output.write_all(&offset.to_le_bytes())?;
            stats.add_offset(1);
        }
        RelativeOffsetLength::TwoBytes => {
            let offset: u16 = relative_offset as u16;
            output.write_all(&offset.to_le_bytes())?;
            stats.add_offset(2);
        }
        RelativeOffsetLength::FourBytes => {
            let offset: u32 = relative_offset as u32;
            output.write_all(&offset.to_le_bytes())?;
            stats.add_offset(4);
        }
        RelativeOffsetLength::EightBytes => {
            output.write_all(&relative_offset.to_le_bytes())?;
            stats.add_offset(8);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn serialize_shaped_directory(
    shape_id: DirectoryShapeId,
    dir: &[(StringId, DirEntryId)],
    object_hash_id: HashId,
    offset: AbsoluteOffset,
    output: &mut SerializeOutput,
    storage: &Storage,
    repository: &mut ContextKeyValueStore,
    stats: &mut SerializeStats,
) -> Result<(), SerializationError> {
    use SerializationError::*;

    let mut nblobs_inlined: usize = 0;
    let mut blobs_length: usize = 0;

    let start = output.len();

    // Replaced by ObjectHeader
    output.write_all(&[0, 0])?;

    serialize_hash_id(object_hash_id, output, repository, stats)?;

    let shape_id = shape_id.as_u32();
    output.write_all(&shape_id.to_le_bytes())?;

    for (_, dir_entry_id) in dir {
        let dir_entry = storage.get_dir_entry(*dir_entry_id)?;

        let kind = dir_entry.dir_entry_kind();

        let blob_inline = dir_entry.get_inlined_blob(storage);
        let blob_inline_length = blob_inline.as_ref().map(|b| b.len()).unwrap_or(0);

        if let Some(blob_inline) = blob_inline {
            let byte: [u8; 1] = DirEntryShapeHeader::new()
                .with_kind(kind)
                .with_offset_length(RelativeOffsetLength::OneByte) // Ignored on deserialization
                .with_blob_inline_length(blob_inline_length as u8)
                .into_bytes();

            output.write_all(&byte[..])?;
            output.write_all(&blob_inline)?;

            nblobs_inlined += 1;
            blobs_length += blob_inline.len();
        } else {
            let dir_entry_offset = dir_entry.get_offset().ok_or(MissingOffset)?;
            let (relative_offset, offset_length) = get_relative_offset(offset, dir_entry_offset);

            let byte: [u8; 1] = DirEntryShapeHeader::new()
                .with_kind(kind)
                .with_offset_length(offset_length)
                .with_blob_inline_length(blob_inline_length as u8)
                .into_bytes();

            output.write_all(&byte[..])?;
            serialize_offset(output, relative_offset, offset_length, stats)?;
        }
    }

    write_object_header(output, start, ObjectTag::ShapedDirectory)?;

    stats.add_shape(nblobs_inlined, blobs_length);

    Ok(())
}

fn write_object_header(
    output: &mut SerializeOutput,
    start: usize,
    tag: ObjectTag,
) -> Result<(), SerializationError> {
    let length = output.len() - start;

    if length <= 0xFF {
        let header: [u8; 1] = ObjectHeader::new()
            .with_tag(tag)
            .with_length(ObjectLength::OneByte)
            .with_is_persistent(true)
            .into_bytes();

        output[start] = header[0];
        output[start + 1] = length as u8;
    } else if length <= (0xFFFF - 1) {
        output.push(0);

        let end = output.len();
        output.copy_within(start + 2..end - 1, start + 3);

        let header: [u8; 1] = ObjectHeader::new()
            .with_tag(tag)
            .with_length(ObjectLength::TwoBytes)
            .with_is_persistent(true)
            .into_bytes();

        let length: u16 = length as u16 + 1;

        output[start] = header[0];
        output[start + 1..start + 3].copy_from_slice(&length.to_le_bytes());
    } else {
        output.write_all(&[0, 0, 0])?;

        let end = output.len();
        output.copy_within(start + 2..end - 3, start + 5);

        let header: [u8; 1] = ObjectHeader::new()
            .with_tag(tag)
            .with_length(ObjectLength::FourBytes)
            .with_is_persistent(true)
            .into_bytes();

        let length: u32 = length as u32 + 3;

        output[start] = header[0];
        output[start + 1..start + 5].copy_from_slice(&length.to_le_bytes());
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn serialize_directory_or_shape(
    dir: &[(StringId, DirEntryId)],
    object_hash_id: HashId,
    offset: AbsoluteOffset,
    output: &mut SerializeOutput,
    storage: &Storage,
    repository: &mut ContextKeyValueStore,
    stats: &mut SerializeStats,
    strings: &StringInterner,
) -> Result<(), SerializationError> {
    if let Some(shape_id) = repository.make_shape(dir)? {
        serialize_shaped_directory(
            shape_id,
            dir,
            object_hash_id,
            offset,
            output,
            storage,
            repository,
            stats,
        )
    } else {
        serialize_directory(
            dir,
            object_hash_id,
            offset,
            output,
            storage,
            repository,
            stats,
            strings,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn serialize_directory(
    dir: &[(StringId, DirEntryId)],
    object_hash_id: HashId,
    offset: AbsoluteOffset,
    output: &mut SerializeOutput,
    storage: &Storage,
    repository: &mut ContextKeyValueStore,
    stats: &mut SerializeStats,
    strings: &StringInterner,
) -> Result<(), SerializationError> {
    use SerializationError::*;

    let mut keys_length: usize = 0;
    let mut nblobs_inlined: usize = 0;
    let mut blobs_length: usize = 0;

    let start = output.len();

    // Replaced by ObjectHeader
    output.write_all(&[0, 0])?;

    serialize_hash_id(object_hash_id, output, repository, stats)?;

    for (key_id, dir_entry_id) in dir {
        let key = strings.get_str(*key_id)?;
        let dir_entry = storage.get_dir_entry(*dir_entry_id)?;

        let kind = dir_entry.dir_entry_kind();

        let blob_inline = dir_entry.get_inlined_blob(storage);
        let blob_inline_length = blob_inline.as_ref().map(|b| b.len()).unwrap_or(0);

        let (relative_offset, offset_length) = match dir_entry
            .get_offset()
            .map(|dir_entry_offset| get_relative_offset(offset, dir_entry_offset))
        {
            Some((relative_offset, offset_length)) => (Some(relative_offset), Some(offset_length)),
            None => (None, None),
        };

        match key.len() {
            len if len != 0 && len < 4 => {
                let byte: [u8; 1] = DirEntryHeader::new()
                    .with_kind(kind)
                    .with_key_inline_length(len as u8)
                    .with_blob_inline_length(blob_inline_length as u8)
                    .with_offset_length(offset_length.unwrap_or(RelativeOffsetLength::OneByte))
                    .into_bytes();

                output.write_all(&byte[..])?;
                output.write_all(key.as_bytes())?;
                keys_length += len;
            }
            len => {
                let byte: [u8; 1] = DirEntryHeader::new()
                    .with_kind(kind)
                    .with_key_inline_length(0)
                    .with_blob_inline_length(blob_inline_length as u8)
                    .with_offset_length(offset_length.unwrap_or(RelativeOffsetLength::OneByte))
                    .into_bytes();
                output.write_all(&byte[..])?;

                let key_length: u16 = len.try_into()?;
                output.write_all(&key_length.to_le_bytes())?;
                output.write_all(key.as_bytes())?;
                keys_length += 2 + key.len();
            }
        }

        if let Some(blob_inline) = blob_inline {
            nblobs_inlined += 1;
            blobs_length += blob_inline.len();

            output.write_all(&blob_inline)?;
        } else {
            let relative_offset = relative_offset.ok_or(MissingOffset)?;
            let offset_length = offset_length.ok_or(MissingOffset)?;

            serialize_offset(output, relative_offset, offset_length, stats)?;
        }
    }

    write_object_header(output, start, ObjectTag::Directory)?;

    stats.add_directory(keys_length, nblobs_inlined, blobs_length);

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn serialize_object(
    object: &Object,
    object_hash_id: HashId,
    output: &mut SerializeOutput,
    storage: &Storage,
    strings: &StringInterner,
    stats: &mut SerializeStats,
    _batch: &mut ChunkedVec<(HashId, InlinedBoxedSlice), { BATCH_CHUNK_CAPACITY }>,
    repository: &mut ContextKeyValueStore,
) -> Result<Option<AbsoluteOffset>, SerializationError> {
    let mut offset = output
        .current_offset()
        .ok_or(SerializationError::MissingOffset)?;
    let start = output.len();

    match object {
        Object::Directory(dir_id) => {
            if let Some(inode_id) = dir_id.get_inode_id() {
                offset = serialize_inode(
                    DirectoryOrInodeId::Inode(inode_id),
                    output,
                    object_hash_id,
                    storage,
                    stats,
                    repository,
                    strings,
                )?;
            } else {
                let dir = storage.get_small_dir(*dir_id)?;

                serialize_directory_or_shape(
                    dir.as_ref(),
                    object_hash_id,
                    offset,
                    output,
                    storage,
                    repository,
                    stats,
                    strings,
                )?;
            }
        }
        Object::Blob(blob_id) => {
            debug_assert!(!blob_id.is_inline());

            let blob = storage.get_blob(*blob_id)?;

            // Replaced by ObjectHeader
            output.write_all(&[0, 0])?;

            serialize_hash_id(object_hash_id, output, repository, stats)?;
            output.write_all(blob.as_ref())?;

            write_object_header(output, start, ObjectTag::Blob)?;

            stats.add_blob(blob.len());
        }
        Object::Commit(commit) => {
            // Replaced by ObjectHeader
            output.write_all(&[0, 0])?;

            serialize_hash_id(object_hash_id, output, repository, stats)?;

            let author_length = match commit.author.len() {
                length if length <= 0xFF => ObjectLength::OneByte,
                length if length <= 0xFFFF => ObjectLength::TwoBytes,
                _ => ObjectLength::FourBytes,
            };

            let (root_relative_offset, root_offset_length) =
                get_relative_offset(offset, commit.root_ref.offset());

            let (is_parent_exist, (parent_relative_offset, parent_offset_length)) =
                match commit.parent_commit_ref {
                    Some(parent) if parent.offset_opt().is_some() => {
                        (true, get_relative_offset(offset, parent.offset()))
                    }
                    Some(_) => {
                        // When creating a snapshot, the parent does have a `HashId` but doesn't
                        // have an offset
                        (true, (0.into(), RelativeOffsetLength::OneByte))
                    }
                    None => (false, (0.into(), RelativeOffsetLength::OneByte)),
                };

            let header: [u8; 1] = CommitHeader::new()
                .with_is_parent_exist(is_parent_exist)
                .with_parent_offset_length(parent_offset_length)
                .with_root_offset_length(root_offset_length)
                .with_author_length(author_length)
                .into_bytes();

            output.write_all(&header)?;

            if let Some(parent) = commit.parent_commit_ref {
                serialize_hash_id(parent.hash_id(), output, repository, stats)?;
                serialize_offset(output, parent_relative_offset, parent_offset_length, stats)?;
            };

            let root_hash_id = commit.root_ref.hash_id();

            serialize_hash_id(root_hash_id, output, repository, stats)?;
            serialize_offset(output, root_relative_offset, root_offset_length, stats)?;

            output.write_all(&commit.time.to_le_bytes())?;

            match author_length {
                ObjectLength::OneByte => {
                    let author_length: u8 = commit.author.len() as u8;
                    output.write_all(&author_length.to_le_bytes())?;
                }
                ObjectLength::TwoBytes => {
                    let author_length: u16 = commit.author.len() as u16;
                    output.write_all(&author_length.to_le_bytes())?;
                }
                ObjectLength::FourBytes => {
                    let author_length: u32 = commit.author.len() as u32;
                    output.write_all(&author_length.to_le_bytes())?;
                }
            }

            output.write_all(commit.author.as_bytes())?;

            // The message length is inferred.
            // It's until the end of the slice
            output.write_all(commit.message.as_bytes())?;

            write_object_header(output, start, ObjectTag::Commit)?;
        }
    };

    stats.total_bytes += output.len() - start;

    Ok(Some(offset))
}

#[derive(Default, Debug)]
struct PointersOffsetsHeader {
    bitfield: u64,
}

impl PointersOffsetsHeader {
    fn set(&mut self, index: usize, offset_length: RelativeOffsetLength) {
        assert!(index < 32);

        let bits: u64 = match offset_length {
            RelativeOffsetLength::OneByte => 0,
            RelativeOffsetLength::TwoBytes => 1,
            RelativeOffsetLength::FourBytes => 2,
            RelativeOffsetLength::EightBytes => 3,
        };

        self.bitfield |= bits << (index * 2);

        assert_eq!(self.get(index), offset_length)
    }

    fn get(&self, index: usize) -> RelativeOffsetLength {
        assert!(index < 32);

        let bits = (self.bitfield >> (index * 2)) & 0b11;

        match bits {
            0 => RelativeOffsetLength::OneByte,
            1 => RelativeOffsetLength::TwoBytes,
            2 => RelativeOffsetLength::FourBytes,
            _ => RelativeOffsetLength::EightBytes,
        }
    }

    /// Sets bits to zero at `index`
    #[cfg(test)]
    fn clear(&mut self, index: usize) {
        self.bitfield &= !(0b11 << (index * 2));
    }

    fn from_pointers(
        object_offset: AbsoluteOffset,
        pointers: PointersId,
        storage: &Storage,
    ) -> Result<Self, SerializationError> {
        let mut bitfield = Self::default();

        for (index, (_, thin_pointer_id)) in pointers.iter().enumerate() {
            let pointer = storage.pointer_copy(thin_pointer_id)?;

            let p_offset = storage
                .pointer_retrieve_offset(&pointer)?
                .ok_or(SerializationError::MissingOffset)?;

            let (_, offset_length) = get_relative_offset(object_offset, p_offset);

            bitfield.set(index, offset_length);
        }

        Ok(bitfield)
    }

    fn to_bytes(&self) -> [u8; 8] {
        self.bitfield.to_le_bytes()
    }

    fn from_bytes(bytes: [u8; 8]) -> Self {
        Self {
            bitfield: u64::from_le_bytes(bytes),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn serialize_inode(
    ptr_id: DirectoryOrInodeId,
    output: &mut SerializeOutput,
    object_hash_id: HashId,
    storage: &Storage,
    stats: &mut SerializeStats,
    repository: &mut ContextKeyValueStore,
    strings: &StringInterner,
) -> Result<AbsoluteOffset, SerializationError> {
    use SerializationError::*;

    let offset;

    match ptr_id {
        DirectoryOrInodeId::Inode(inode_id) => {
            let Inode {
                depth,
                nchildren,
                pointers,
            } = storage.get_inode(inode_id)?;

            let depth: u32 = *depth as u32;

            stats.add_inode_pointers();

            // Recursively serialize all children
            for (_, thin_pointer_id) in pointers.iter() {
                let pointer = storage.pointer_copy(thin_pointer_id)?;

                if pointer.is_commited() {
                    // We only want to serialize new inodes.
                    // We skip inodes that were previously serialized and already
                    // in the repository.
                    continue;
                }

                let hash_id = storage
                    .pointer_retrieve_hashid(&pointer, repository)?
                    .ok_or(MissingHashId)?;

                let ptr_id = pointer.ptr_id().ok_or(MissingInodeId)?;

                if storage.pointer_retrieve_offset(&pointer)?.is_some() {
                    // The child has already been serialized, skipt it.
                    // This is a different case than `pointer.is_commited()` above, here
                    // the child is a duplicate.
                    continue;
                }

                let offset =
                    serialize_inode(ptr_id, output, hash_id, storage, stats, repository, strings)?;

                storage.pointer_set_offset(&pointer, offset)?;
            }

            let start = output.len();
            offset = output
                .current_offset()
                .ok_or(SerializationError::MissingOffset)?;

            // Replaced by ObjectHeader
            output.write_all(&[0, 0])?;

            serialize_hash_id(object_hash_id, output, repository, stats)?;

            output.write_all(&depth.to_le_bytes())?;
            output.write_all(&nchildren.to_le_bytes())?;

            let bitfield: PointersBitfield = pointers.bitfield();
            let bitfield: [u8; 4] = bitfield.to_bytes();
            output.write_all(&bitfield)?;

            let bitfield_offsets =
                PointersOffsetsHeader::from_pointers(offset, *pointers, storage)?;
            output.write_all(&bitfield_offsets.to_bytes())?;

            for (_, thin_pointer_id) in pointers.iter() {
                let pointer = storage.pointer_copy(thin_pointer_id)?;

                let pointer_offset = storage
                    .pointer_retrieve_offset(&pointer)?
                    .ok_or(SerializationError::MissingOffset)?;

                let (relative_offset, offset_length) = get_relative_offset(offset, pointer_offset);

                serialize_offset(output, relative_offset, offset_length, stats)?;
            }

            write_object_header(output, start, ObjectTag::InodePointers)?;
        }
        DirectoryOrInodeId::Directory(dir_id) => {
            // We don't check if it's a new inode because the parent
            // caller (recursively) confirmed it's a new one.

            offset = output
                .current_offset()
                .ok_or(SerializationError::MissingOffset)?;

            let dir = storage.get_small_dir(dir_id)?;
            serialize_directory_or_shape(
                dir.as_ref(),
                object_hash_id,
                offset,
                output,
                storage,
                repository,
                stats,
                strings,
            )?;
        }
    };

    Ok(offset)
}

fn deserialize_offset(
    data: &[u8],
    offset_length: RelativeOffsetLength,
    object_offset: AbsoluteOffset,
) -> Result<(AbsoluteOffset, usize), DeserializationError> {
    use DeserializationError::*;

    let object_offset = object_offset.as_u64();

    match offset_length {
        RelativeOffsetLength::OneByte => {
            let byte = data.get(0).ok_or(UnexpectedEOF)?;
            let relative_offset: u8 = u8::from_le_bytes([*byte]);
            let absolute_offset: u64 = object_offset - relative_offset as u64;
            Ok((absolute_offset.into(), 1))
        }
        RelativeOffsetLength::TwoBytes => {
            let bytes = data.get(..2).ok_or(UnexpectedEOF)?;
            let relative_offset: u16 = u16::from_le_bytes(bytes.try_into()?);
            let absolute_offset: u64 = object_offset - relative_offset as u64;
            Ok((absolute_offset.into(), 2))
        }
        RelativeOffsetLength::FourBytes => {
            let bytes = data.get(..4).ok_or(UnexpectedEOF)?;
            let relative_offset: u32 = u32::from_le_bytes(bytes.try_into()?);
            let absolute_offset: u64 = object_offset - relative_offset as u64;
            Ok((absolute_offset.into(), 4))
        }
        RelativeOffsetLength::EightBytes => {
            let bytes = data.get(..8).ok_or(UnexpectedEOF)?;
            let relative_offset: u64 = u64::from_le_bytes(bytes.try_into()?);
            let absolute_offset: u64 = object_offset - relative_offset;
            Ok((absolute_offset.into(), 8))
        }
    }
}

fn deserialize_shaped_directory(
    data: &[u8],
    object_offset: AbsoluteOffset,
    storage: &mut Storage,
    repository: &ContextKeyValueStore,
    strings: &mut StringInterner,
) -> Result<DirectoryId, DeserializationError> {
    use DeserializationError as Error;
    use DeserializationError::*;

    let mut pos = 0;
    let data_length = data.len();

    let shape_id = data.get(pos..pos + 4).ok_or(UnexpectedEOF)?;
    let shape_id = u32::from_le_bytes(shape_id.try_into()?);
    let shape_id = DirectoryShapeId::from(shape_id);

    let directory_shape = match repository.get_shape(shape_id).map_err(Box::new)? {
        ShapeStrings::SliceIds(slice_ids) => slice_ids,
        ShapeStrings::Owned(slice_strings) => {
            // We are in the readonly protocol runner.
            // Store the `String` in the `StringInterner`.
            let string_ids: Vec<StringId> = slice_strings
                .iter()
                .map(|s| strings.make_string_id(s))
                .collect();
            Cow::Owned(string_ids)
        }
    };

    let mut directory_shape = directory_shape.as_ref().iter();

    pos += 4;

    let dir_id = storage.with_new_dir::<_, Result<_, Error>>(|storage, new_dir| {
        while pos < data_length {
            let dir_entry_header = data.get(pos..pos + 1).ok_or(UnexpectedEOF)?;
            let dir_entry_header = DirEntryShapeHeader::from_bytes([dir_entry_header[0]; 1]);

            pos += 1;

            let key_id = directory_shape.next().copied().ok_or(CannotFindNextShape)?;

            let kind = dir_entry_header.kind();
            let blob_inline_length = dir_entry_header.blob_inline_length() as usize;
            let offset_length: RelativeOffsetLength = dir_entry_header.offset_length();

            let dir_entry = if blob_inline_length > 0 {
                // The blob is inlined

                let blob = data
                    .get(pos..pos + blob_inline_length)
                    .ok_or(UnexpectedEOF)?;
                let blob_id = storage.add_blob_by_ref(blob)?;

                pos += blob_inline_length;

                DirEntry::new_commited(kind, None, Some(Object::Blob(blob_id)))
            } else {
                let (absolute_offset, nbytes) =
                    deserialize_offset(&data[pos..], offset_length, object_offset)?;

                pos += nbytes;

                let dir_entry = DirEntry::new_commited(kind, None, None);
                dir_entry.set_offset(absolute_offset);
                dir_entry
            };

            let dir_entry_id = storage.add_dir_entry(dir_entry)?;

            new_dir.push((key_id, dir_entry_id));
        }

        Ok(storage.append_to_directories(new_dir))
    })??;

    Ok(dir_id)
}

fn deserialize_directory(
    data: &[u8],
    object_offset: AbsoluteOffset,
    storage: &mut Storage,
    strings: &mut StringInterner,
) -> Result<DirectoryId, DeserializationError> {
    use DeserializationError as Error;
    use DeserializationError::*;

    let mut pos = 0;
    let data_length = data.len();

    let dir_id = storage.with_new_dir::<_, Result<_, Error>>(|storage, new_dir| {
        while pos < data_length {
            let dir_entry_header = data.get(pos..pos + 1).ok_or(UnexpectedEOF)?;
            let dir_entry_header = DirEntryHeader::from_bytes([dir_entry_header[0]; 1]);

            pos += 1;

            let key_id = match dir_entry_header.key_inline_length() as usize {
                len if len > 0 => {
                    // The key is in the next `len` bytes
                    let key_bytes = data.get(pos..pos + len).ok_or(UnexpectedEOF)?;
                    let key_str = std::str::from_utf8(key_bytes)?;
                    pos += len;
                    strings.make_string_id(key_str)
                }
                _ => {
                    // The key length is in 2 bytes, followed by the key itself
                    let key_length = data.get(pos..pos + 2).ok_or(UnexpectedEOF)?;
                    let key_length = u16::from_le_bytes(key_length.try_into()?);
                    let key_length = key_length as usize;

                    let key_bytes = data
                        .get(pos + 2..pos + 2 + key_length)
                        .ok_or(UnexpectedEOF)?;
                    let key_str = std::str::from_utf8(key_bytes)?;
                    pos += 2 + key_length;
                    strings.make_string_id(key_str)
                }
            };

            let kind = dir_entry_header.kind();
            let blob_inline_length = dir_entry_header.blob_inline_length() as usize;
            let offset_length = dir_entry_header.offset_length();

            let dir_entry = if blob_inline_length > 0 {
                // The blob is inlined

                let blob = data
                    .get(pos..pos + blob_inline_length)
                    .ok_or(UnexpectedEOF)?;
                let blob_id = storage.add_blob_by_ref(blob)?;

                pos += blob_inline_length;

                DirEntry::new_commited(kind, None, Some(Object::Blob(blob_id)))
            } else {
                let bytes = data.get(pos..).ok_or(UnexpectedEOF)?;
                let (absolute_offset, nbytes) =
                    deserialize_offset(bytes, offset_length, object_offset)?;

                pos += nbytes;

                let dir_entry = DirEntry::new_commited(kind, None, None);
                dir_entry.set_offset(absolute_offset);
                dir_entry
            };

            let dir_entry_id = storage.add_dir_entry(dir_entry)?;

            new_dir.push((key_id, dir_entry_id));
        }

        Ok(storage.append_to_directories(new_dir))
    })??;

    Ok(dir_id)
}

pub fn read_object_length(
    data: &[u8],
    header: &ObjectHeader,
) -> Result<(usize, usize), DeserializationError> {
    use DeserializationError::*;

    match header.length() {
        ObjectLength::OneByte => {
            let length = data.get(1).copied().ok_or(UnexpectedEOF)? as usize;
            Ok((1 + 1, length))
        }
        ObjectLength::TwoBytes => {
            let length = data.get(1..3).ok_or(UnexpectedEOF)?;
            let length = u16::from_le_bytes(length.try_into()?) as usize;
            Ok((1 + 2, length))
        }
        ObjectLength::FourBytes => {
            let length = data.get(1..5).ok_or(UnexpectedEOF)?;
            let length = u32::from_le_bytes(length.try_into()?) as usize;
            Ok((1 + 4, length))
        }
    }
}

/// Extract values from `data` to store them in `storage`.
/// Return an `Object`, which can be ids (refering to data inside `storage`) or a `Commit`
pub fn deserialize_object(
    bytes: &[u8],
    object_offset: AbsoluteOffset,
    storage: &mut Storage,
    strings: &mut StringInterner,
    repository: &ContextKeyValueStore,
) -> Result<Object, DeserializationError> {
    use DeserializationError::*;

    let (header, bytes) = deserialize_object_header(bytes, object_offset, storage)?;

    match header.tag_or_err().map_err(|_| UnknownID)? {
        ObjectTag::Directory => {
            deserialize_directory(bytes, object_offset, storage, strings).map(Object::Directory)
        }
        ObjectTag::ShapedDirectory => {
            deserialize_shaped_directory(bytes, object_offset, storage, repository, strings)
                .map(Object::Directory)
        }
        ObjectTag::Blob => storage
            .add_blob_by_ref(bytes)
            .map(Object::Blob)
            .map_err(Into::into),
        ObjectTag::InodePointers => {
            let ptr_id = deserialize_inode_pointers(bytes, object_offset, storage)?;

            Ok(Object::Directory(ptr_id.into_dir()))
        }
        ObjectTag::Commit => {
            let mut pos = 0;

            let header = bytes.get(pos).ok_or(UnexpectedEOF)?;
            let header = CommitHeader::from_bytes([*header; 1]);

            pos += 1;

            let parent_commit_ref = if header.is_parent_exist() {
                let data = bytes.get(pos..).ok_or(UnexpectedEOF)?;
                let (parent_commit_hash, nbytes) = deserialize_hash_id(data)?;

                pos += nbytes;

                let data = bytes.get(pos..).ok_or(UnexpectedEOF)?;
                let (parent_absolute_offset, nbytes) =
                    deserialize_offset(data, header.parent_offset_length(), object_offset)?;

                pos += nbytes;

                Some(ObjectReference::new(
                    parent_commit_hash,
                    Some(parent_absolute_offset),
                ))
            } else {
                None
            };

            let data = bytes.get(pos..).ok_or(UnexpectedEOF)?;
            let (root_hash, nbytes) = deserialize_hash_id(data)?;

            pos += nbytes;

            let data = bytes.get(pos..).ok_or(UnexpectedEOF)?;
            let (root_absolute_offset, nbytes) =
                deserialize_offset(data, header.root_offset_length(), object_offset)?;

            let root_ref = ObjectReference::new(
                Some(root_hash.ok_or(MissingRootHash)?),
                Some(root_absolute_offset),
            );

            pos += nbytes;

            let time = bytes.get(pos..pos + 8).ok_or(UnexpectedEOF)?;
            let time = u64::from_le_bytes(time.try_into()?);

            pos += 8;

            let author_length: usize = match header.author_length() {
                ObjectLength::OneByte => {
                    let author_length: u8 = *bytes.get(pos).ok_or(UnexpectedEOF)?;
                    pos += 1;
                    author_length as usize
                }
                ObjectLength::TwoBytes => {
                    let author_length = bytes.get(pos..pos + 2).ok_or(UnexpectedEOF)?;
                    pos += 2;
                    u16::from_le_bytes(author_length.try_into()?) as usize
                }
                ObjectLength::FourBytes => {
                    let author_length = bytes.get(pos..pos + 4).ok_or(UnexpectedEOF)?;
                    pos += 4;
                    u32::from_le_bytes(author_length.try_into()?) as usize
                }
            };

            let author = bytes.get(pos..pos + author_length).ok_or(UnexpectedEOF)?;
            let author = author.to_vec();

            pos += author_length;

            let message = bytes.get(pos..).ok_or(UnexpectedEOF)?;
            let message = message.to_vec();

            Ok(Object::Commit(Box::new(Commit {
                parent_commit_ref,
                root_ref,
                time,
                author: String::from_utf8(author)?,
                message: String::from_utf8(message)?,
            })))
        }
    }
}

fn deserialize_inode_pointers(
    data: &[u8],
    object_offset: AbsoluteOffset,
    storage: &mut Storage,
) -> Result<DirectoryOrInodeId, DeserializationError> {
    use DeserializationError::*;

    let mut pos = 0;

    let depth = data.get(pos..pos + 4).ok_or(UnexpectedEOF)?;
    let depth = u32::from_le_bytes(depth.try_into()?);

    let nchildren = data.get(pos + 4..pos + 8).ok_or(UnexpectedEOF)?;
    let nchildren = u32::from_le_bytes(nchildren.try_into()?);

    pos += 8;

    let pointers_bitfield = data.get(pos..pos + 4).ok_or(UnexpectedEOF)?;
    let pointers_bitfield = PointersBitfield::from_bytes(pointers_bitfield.try_into()?);

    let indexes_iter = pointers_bitfield.iter();

    pos += 4;

    let offsets_header = data.get(pos..pos + 8).ok_or(UnexpectedEOF)?;
    let offsets_header = PointersOffsetsHeader::from_bytes(offsets_header.try_into()?);

    pos += 8;

    let mut pointers: [Option<PointerOnStack>; 32] = Default::default();

    for (index, pointer_index) in indexes_iter.enumerate() {
        let offset_length = offsets_header.get(index);
        let (absolute_offset, nbytes) =
            deserialize_offset(&data[pos..], offset_length, object_offset)?;

        pos += nbytes;

        pointers[pointer_index as usize] = Some(PointerOnStack {
            thin_pointer: None,
            fat_pointer: FatPointer::new_commited(None, Some(absolute_offset)),
        });
    }

    Ok(storage.add_inode_pointers(depth as u16, nchildren, pointers)?)
}

fn deserialize_object_header<'a>(
    data: &'a [u8],
    object_offset: AbsoluteOffset,
    storage: &mut Storage,
) -> Result<(ObjectHeader, &'a [u8]), DeserializationError> {
    use DeserializationError::*;

    let header = data.get(0).copied().ok_or(UnexpectedEOF)?;
    let header: ObjectHeader = ObjectHeader::from_bytes([header]);

    let (header_nbytes, object_length) = read_object_length(data, &header)?;
    let data = data
        .get(header_nbytes..object_length)
        .ok_or(UnexpectedEOF)?;

    let (object_hash_id, nbytes) = deserialize_hash_id(data)?;
    storage
        .offsets_to_hash_id
        .insert(object_offset, object_hash_id.ok_or(MissingHash)?);

    let data = data.get(nbytes..).ok_or(UnexpectedEOF)?;

    Ok((header, data))
}

pub fn deserialize_inode(
    data: &[u8],
    object_offset: AbsoluteOffset,
    storage: &mut Storage,
    repository: &ContextKeyValueStore,
    strings: &mut StringInterner,
) -> Result<DirectoryOrInodeId, DeserializationError> {
    use DeserializationError::*;

    let (header, data) = deserialize_object_header(data, object_offset, storage)?;

    match header.tag_or_err().map_err(|_| UnknownID)? {
        ObjectTag::InodePointers => {
            let ptr_id = deserialize_inode_pointers(data, object_offset, storage)?;
            Ok(ptr_id)
        }
        ObjectTag::Directory => {
            let dir_id = deserialize_directory(data, object_offset, storage, strings)?;
            Ok(DirectoryOrInodeId::Directory(dir_id))
        }
        ObjectTag::ShapedDirectory => {
            let dir_id =
                deserialize_shaped_directory(data, object_offset, storage, repository, strings)?;
            Ok(DirectoryOrInodeId::Directory(dir_id))
        }
        _ => Err(UnknownID),
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use tezos_timing::SerializeStats;

    use crate::kv_store::persistent::PersistentConfiguration;
    use crate::persistent::KeyValueStoreBackend;
    use crate::working_tree::string_interner::StringInterner;
    use crate::{
        hash::hash_object, kv_store::persistent::Persistent, working_tree::storage::DirectoryId,
    };

    use super::*;

    #[test]
    fn test_serialize() {
        let mut storage = Storage::new();
        let mut strings = StringInterner::default();
        let mut repo = Persistent::try_new(PersistentConfiguration {
            db_path: None,
            startup_check: true,
            read_mode: false,
        })
        .unwrap();
        let mut stats = SerializeStats::default();
        let mut batch = ChunkedVec::<_, BATCH_CHUNK_CAPACITY>::default();
        let fake_hash_id = HashId::try_from(1).unwrap();

        let offset = repo.synchronize_data(&[], &[0, 0, 0, 0, 0, 0]).unwrap();

        // Test Object::Directory

        let dir_id = DirectoryId::empty();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "a",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(1.into()),
                &mut strings,
                &repo,
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "bab",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(1.into()),
                &mut strings,
                &repo,
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(1.into()),
                &mut strings,
                &repo,
            )
            .unwrap();

        let mut bytes = SerializeOutput::new(offset);
        let offset = serialize_object(
            &Object::Directory(dir_id),
            fake_hash_id,
            &mut bytes,
            &storage,
            &strings,
            &mut stats,
            &mut batch,
            &mut repo,
        )
        .unwrap();

        let object =
            deserialize_object(&bytes, offset.unwrap(), &mut storage, &mut strings, &repo).unwrap();

        if let Object::Directory(object) = object {
            assert_eq!(
                storage.get_owned_dir(dir_id, &mut strings, &repo).unwrap(),
                storage.get_owned_dir(object, &mut strings, &repo).unwrap()
            )
        } else {
            panic!();
        }

        // Test Object::Directory (Shaped)

        let dir_id = DirectoryId::empty();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "a",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(1.into()),
                &mut strings,
                &repo,
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "bab",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(2.into()),
                &mut strings,
                &repo,
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "0aa",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(3.into()),
                &mut strings,
                &repo,
            )
            .unwrap();

        repo.synchronize_data(&[], &bytes).unwrap();
        bytes.clear();

        let offset = serialize_object(
            &Object::Directory(dir_id),
            fake_hash_id,
            &mut bytes,
            &storage,
            &strings,
            &mut stats,
            &mut batch,
            &mut repo,
        )
        .unwrap();

        let object =
            deserialize_object(&bytes, offset.unwrap(), &mut storage, &mut strings, &repo).unwrap();

        if let Object::Directory(object) = object {
            assert_eq!(
                storage.get_owned_dir(dir_id, &mut strings, &repo).unwrap(),
                storage.get_owned_dir(object, &mut strings, &repo).unwrap()
            )
        } else {
            panic!();
        }

        // Test Object::Directory (Shaped)

        let dir_id = DirectoryId::empty();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "a1",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(1.into()),
                &mut strings,
                &repo,
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "bab1",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(2.into()),
                &mut strings,
                &repo,
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "0aa1",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(3.into()),
                &mut strings,
                &repo,
            )
            .unwrap();

        repo.synchronize_data(&[], &bytes).unwrap();
        bytes.clear();

        let offset = serialize_object(
            &Object::Directory(dir_id),
            fake_hash_id,
            &mut bytes,
            &storage,
            &strings,
            &mut stats,
            &mut batch,
            &mut repo,
        )
        .unwrap();

        let object =
            deserialize_object(&bytes, offset.unwrap(), &mut storage, &mut strings, &repo).unwrap();

        if let Object::Directory(object) = object {
            assert_eq!(
                storage.get_owned_dir(dir_id, &mut strings, &repo).unwrap(),
                storage.get_owned_dir(object, &mut strings, &repo).unwrap()
            )
        } else {
            panic!();
        }

        // Test Object::Blob

        // Not inlined value
        let blob_id = storage.add_blob_by_ref(&[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();

        repo.synchronize_data(&[], &bytes).unwrap();
        bytes.clear();

        let offset = serialize_object(
            &Object::Blob(blob_id),
            fake_hash_id,
            &mut bytes,
            &storage,
            &strings,
            &mut stats,
            &mut batch,
            &mut repo,
        )
        .unwrap();

        let object =
            deserialize_object(&bytes, offset.unwrap(), &mut storage, &mut strings, &repo).unwrap();
        if let Object::Blob(object) = object {
            let blob = storage.get_blob(object).unwrap();
            assert_eq!(blob.as_ref(), &[1, 2, 3, 4, 5, 6, 7, 8]);
        } else {
            panic!();
        }

        // Test Object::Commit

        repo.synchronize_data(&[], &bytes).unwrap();
        bytes.clear();

        let commit = Commit {
            parent_commit_ref: Some(ObjectReference::new(HashId::new(9876), Some(1.into()))),
            root_ref: ObjectReference::new(HashId::new(12345), Some(2.into())),
            time: 123456,
            author: "123".to_string(),
            message: "abc".to_string(),
        };

        let offset = serialize_object(
            &Object::Commit(Box::new(commit.clone())),
            fake_hash_id,
            &mut bytes,
            &storage,
            &strings,
            &mut stats,
            &mut batch,
            &mut repo,
        )
        .unwrap();

        let object =
            deserialize_object(&bytes, offset.unwrap(), &mut storage, &mut strings, &repo).unwrap();
        if let Object::Commit(object) = object {
            assert_eq!(*object, commit);
        } else {
            panic!();
        }

        // Test Object::Commit with no parent

        repo.synchronize_data(&[], &bytes).unwrap();
        bytes.clear();

        let commit = Commit {
            parent_commit_ref: None,
            root_ref: ObjectReference::new(HashId::new(12), Some(3.into())),
            time: 1234567,
            author: "123456".repeat(100),
            message: "abcd".repeat(100),
        };

        let offset = serialize_object(
            &Object::Commit(Box::new(commit.clone())),
            fake_hash_id,
            &mut bytes,
            &storage,
            &strings,
            &mut stats,
            &mut batch,
            &mut repo,
        )
        .unwrap();

        let object =
            deserialize_object(&bytes, offset.unwrap(), &mut storage, &mut strings, &repo).unwrap();
        if let Object::Commit(object) = object {
            assert_eq!(*object, commit);
        } else {
            panic!();
        }

        repo.synchronize_data(&[], &bytes).unwrap();
        bytes.clear();

        let mut offsets = Vec::with_capacity(32);

        // Test Inode::Directory

        let mut pointers: [Option<PointerOnStack>; 32] = Default::default();

        for (index, pointer) in pointers.iter_mut().enumerate() {
            storage
                .dir_insert(
                    dir_id,
                    "_",
                    DirEntry::new_commited(DirEntryKind::Blob, None, None),
                    &mut strings,
                    &repo,
                )
                .unwrap();

            let dir_id = DirectoryId::try_new_dir(index, index).unwrap();
            let ptr_id = DirectoryOrInodeId::Directory(dir_id);

            let hash_id = HashId::new((index + 1) as u64).unwrap();

            let offset = serialize_inode(
                ptr_id, &mut bytes, hash_id, &storage, &mut stats, &mut repo, &strings,
            )
            .unwrap();

            offsets.push(offset);

            let fat_pointer = FatPointer::new_commited(None, Some(offset));

            *pointer = Some(PointerOnStack {
                thin_pointer: None,
                fat_pointer,
            });
        }

        let inode = storage.add_inode_pointers(100, 200, pointers).unwrap();

        let hash_id = HashId::new(123).unwrap();

        let offset = serialize_inode(
            inode, &mut bytes, hash_id, &storage, &mut stats, &mut repo, &strings,
        )
        .unwrap();

        repo.synchronize_data(&[], &bytes).unwrap();
        bytes.clear();

        let mut buffer = Vec::with_capacity(1000);
        let inode_bytes = repo
            .get_object_bytes(ObjectReference::new(None, Some(offset)), &mut buffer)
            .unwrap();

        let new_inode_id =
            deserialize_inode(inode_bytes, offset, &mut storage, &repo, &mut strings).unwrap();

        if let DirectoryOrInodeId::Inode(inode_id) = new_inode_id {
            let Inode {
                depth,
                nchildren,
                pointers,
            } = storage.get_inode(inode_id).unwrap();

            let npointers = pointers.npointers();

            assert_eq!(*depth, 100);
            assert_eq!(*nchildren, 200);
            assert_eq!(npointers, 32);

            for (index, (_, thin_pointer_id)) in pointers.iter().enumerate() {
                let fat_ptr = storage.pointer_copy(thin_pointer_id).unwrap();

                let ptr_data = fat_ptr.get_data().unwrap().unwrap();
                assert_eq!(ptr_data.offset(), offsets[index]);
            }
        } else {
            panic!()
        }

        repo.synchronize_data(&[], &bytes).unwrap();
        bytes.clear();

        // Test Inode::Value

        let dir_id = DirectoryId::empty();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "a",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(1.into()),
                &mut strings,
                &repo,
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "bab",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(2.into()),
                &mut strings,
                &repo,
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(3.into()),
                &mut strings,
                &repo,
            )
            .unwrap();

        let inode_id = DirectoryOrInodeId::Directory(dir_id);

        let offset = serialize_inode(
            inode_id, &mut bytes, hash_id, &storage, &mut stats, &mut repo, &strings,
        )
        .unwrap();

        repo.synchronize_data(&[], &bytes).unwrap();
        let mut buffer = Vec::with_capacity(1000);
        let inode_bytes = repo
            .get_object_bytes(ObjectReference::new(None, Some(offset)), &mut buffer)
            .unwrap();

        let new_inode_id =
            deserialize_inode(inode_bytes, offset, &mut storage, &repo, &mut strings).unwrap();

        if let DirectoryOrInodeId::Directory(new_dir_id) = new_inode_id {
            assert_eq!(storage.dir_len(new_dir_id).unwrap(), 3);
            assert_eq!(
                storage.get_owned_dir(dir_id, &mut strings, &repo).unwrap(),
                storage
                    .get_owned_dir(new_dir_id, &mut strings, &repo)
                    .unwrap()
            )
        } else {
            panic!()
        }
    }

    #[test]
    fn test_serialize_empty_blob() {
        let mut repo = Persistent::try_new(PersistentConfiguration {
            db_path: None,
            startup_check: true,
            read_mode: false,
        })
        .expect("failed to create context");
        let mut storage = Storage::new();
        let mut strings = StringInterner::default();
        let mut stats = SerializeStats::default();
        let mut batch = ChunkedVec::<_, BATCH_CHUNK_CAPACITY>::default();

        let fake_hash_id = HashId::try_from(1).unwrap();

        let blob_id = storage.add_blob_by_ref(&[]).unwrap();
        let blob = Object::Blob(blob_id);
        let blob_hash_id = hash_object(&blob, &mut repo, &storage, &strings).unwrap();

        assert!(blob_hash_id.is_some());

        let dir_id = DirectoryId::empty();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "a",
                DirEntry::new_commited(DirEntryKind::Blob, None, None).with_offset(1.into()),
                &mut strings,
                &repo,
            )
            .unwrap();

        let mut bytes = SerializeOutput::new(Some(1.into()));

        let offset = serialize_object(
            &Object::Directory(dir_id),
            fake_hash_id,
            &mut bytes,
            &storage,
            &strings,
            &mut stats,
            &mut batch,
            &mut repo,
        )
        .unwrap();

        let object =
            deserialize_object(&bytes, offset.unwrap(), &mut storage, &mut strings, &repo).unwrap();

        if let Object::Directory(object) = object {
            assert_eq!(
                storage.get_owned_dir(dir_id, &mut strings, &repo).unwrap(),
                storage.get_owned_dir(object, &mut strings, &repo).unwrap()
            )
        } else {
            panic!();
        }
    }

    #[test]
    fn test_hash_id() {
        let mut repo = Persistent::try_new(PersistentConfiguration {
            db_path: None,
            startup_check: true,
            read_mode: false,
        })
        .expect("failed to create context");
        let mut output = SerializeOutput::new(None);
        let mut stats = Default::default();

        let number = HashId::new(10101).unwrap();

        serialize_hash_id(number, &mut output, &mut repo, &mut stats).unwrap();
        let (hash_id, size) = deserialize_hash_id(&output).unwrap();
        assert_eq!(output.len(), 4);
        assert_eq!(hash_id.unwrap(), number);
        assert_eq!(size, 4);

        output.clear();

        let number = HashId::new((u32::MAX as u64) + 10).unwrap();

        serialize_hash_id(number, &mut output, &mut repo, &mut stats).unwrap();
        let (hash_id, size) = deserialize_hash_id(&output).unwrap();
        assert_eq!(output.len(), 6);
        assert_eq!(hash_id.unwrap(), number);
        assert_eq!(size, 6);

        output.clear();

        let number = HashId::new(u32::MAX as u64).unwrap();

        serialize_hash_id(number, &mut output, &mut repo, &mut stats).unwrap();
        let (hash_id, size) = deserialize_hash_id(&output).unwrap();
        assert_eq!(output.len(), 6);
        assert_eq!(hash_id.unwrap(), number);
        assert_eq!(size, 6);
    }

    #[test]
    fn test_inode_headers() {
        let mut header = PointersOffsetsHeader::default();

        for index in 0..32 {
            header.set(index, RelativeOffsetLength::OneByte);
            assert_eq!(header.get(index), RelativeOffsetLength::OneByte);

            header.clear(index);
            header.set(index, RelativeOffsetLength::TwoBytes);
            assert_eq!(header.get(index), RelativeOffsetLength::TwoBytes);

            header.clear(index);
            header.set(index, RelativeOffsetLength::FourBytes);
            assert_eq!(header.get(index), RelativeOffsetLength::FourBytes);

            header.clear(index);
            header.set(index, RelativeOffsetLength::EightBytes);
            assert_eq!(header.get(index), RelativeOffsetLength::EightBytes);
        }
    }
}
