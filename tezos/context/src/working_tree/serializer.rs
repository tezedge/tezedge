// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Serialization/deserialization for objects in the Working Tree so that they can be
//! saved/loaded to/from the repository.

use std::{
    array::TryFromSliceError, convert::TryInto, io::Write, num::TryFromIntError, str::Utf8Error,
    string::FromUtf8Error, sync::Arc,
};

use modular_bitfield::prelude::*;
use static_assertions::assert_eq_size;
use tezos_timing::SerializeStats;

use crate::{
    kv_store::HashId,
    working_tree::{
        storage::{DirectoryId, Inode, PointerToInode},
        Commit, NodeKind,
    },
    ContextKeyValueStore,
};

use super::{
    storage::{Blob, InodeId, NodeId, NodeIdError, Storage, StorageError},
    string_interner::StringId,
    Node, Object,
};

const ID_DIRECTORY: u8 = 0;
const ID_BLOB: u8 = 1;
const ID_COMMIT: u8 = 2;
const ID_INODE_POINTERS: u8 = 3;
const ID_INODE_DIRECTORY: u8 = 4;

const COMPACT_HASH_ID_BIT: u32 = 1 << 23;

const FULL_31_BITS: u32 = 0x7FFFFFFF;
const FULL_23_BITS: u32 = 0x7FFFFF;

#[derive(Debug)]
pub enum SerializationError {
    IOError,
    DirNotFound,
    NodeNotFound,
    BlobNotFound,
    TryFromIntError,
    StorageIdError { error: StorageError },
    HashIdTooBig,
    MissingHashId,
}

impl From<std::io::Error> for SerializationError {
    fn from(_: std::io::Error) -> Self {
        Self::IOError
    }
}

impl From<TryFromIntError> for SerializationError {
    fn from(_: TryFromIntError) -> Self {
        Self::TryFromIntError
    }
}

impl From<StorageError> for SerializationError {
    fn from(error: StorageError) -> Self {
        Self::StorageIdError { error }
    }
}

fn get_inline_blob<'a>(storage: &'a Storage, node: &Node) -> Option<Blob<'a>> {
    if let Some(Object::Blob(blob_id)) = node.get_object() {
        if blob_id.is_inline() {
            return storage.get_blob(blob_id).ok();
        }
    }
    None
}

#[bitfield(bits = 8)]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub struct KeyNodeDescriptor {
    kind: NodeKind,
    blob_inline_length: B3,
    key_inline_length: B4,
}

// Must fit in 1 byte
assert_eq_size!(KeyNodeDescriptor, u8);

fn serialize_directory(
    dir: &[(StringId, NodeId)],
    output: &mut Vec<u8>,
    storage: &Storage,
    stats: &mut SerializeStats,
) -> Result<(), SerializationError> {
    let mut keys_length: usize = 0;
    let mut hash_ids_length: usize = 0;
    let mut highest_hash_id: u32 = 0;
    let mut nblobs_inlined: usize = 0;
    let mut blobs_length: usize = 0;

    for (key_id, node_id) in dir {
        let key = storage.get_str(*key_id)?;

        let node = storage.get_node(*node_id)?;

        let hash_id: u32 = node.hash_id().map(|h| h.as_u32()).unwrap_or(0);
        let kind = node.node_kind();

        let blob_inline = get_inline_blob(storage, &node);
        let blob_inline_length = blob_inline.as_ref().map(|b| b.len()).unwrap_or(0);

        match key.len() {
            len if len != 0 && len < 16 => {
                let byte: [u8; 1] = KeyNodeDescriptor::new()
                    .with_kind(kind)
                    .with_key_inline_length(len as u8)
                    .with_blob_inline_length(blob_inline_length as u8)
                    .into_bytes();
                output.write_all(&byte[..])?;
                output.write_all(key.as_bytes())?;
                keys_length += len;
            }
            len => {
                let byte: [u8; 1] = KeyNodeDescriptor::new()
                    .with_kind(kind)
                    .with_key_inline_length(0)
                    .with_blob_inline_length(blob_inline_length as u8)
                    .into_bytes();
                output.write_all(&byte[..])?;

                let key_length: u16 = len.try_into()?;
                output.write_all(&key_length.to_ne_bytes())?;
                output.write_all(key.as_bytes())?;
                keys_length += 2 + key.len();
            }
        }

        if let Some(blob_inline) = blob_inline {
            nblobs_inlined += 1;
            blobs_length += blob_inline.len();

            output.write_all(&blob_inline)?;
        } else {
            let nbytes = serialize_hash_id(hash_id, output)?;

            hash_ids_length += nbytes;
            highest_hash_id = highest_hash_id.max(hash_id);
        }
    }

    stats.add_directory(
        hash_ids_length,
        keys_length,
        highest_hash_id,
        nblobs_inlined,
        blobs_length,
    );

    Ok(())
}

pub fn serialize_object(
    object: &Object,
    object_hash_id: HashId,
    output: &mut Vec<u8>,
    storage: &Storage,
    stats: &mut SerializeStats,
    batch: &mut Vec<(HashId, Arc<[u8]>)>,
    referenced_older_entries: &mut Vec<HashId>,
) -> Result<(), SerializationError> {
    output.clear();

    match object {
        Object::Directory(dir_id) => {
            if let Some(inode_id) = dir_id.get_inode_id() {
                serialize_inode(
                    inode_id,
                    output,
                    object_hash_id,
                    storage,
                    stats,
                    batch,
                    referenced_older_entries,
                )?;
            } else {
                output.write_all(&[ID_DIRECTORY])?;
                let dir = storage.get_small_dir(*dir_id)?;

                serialize_directory(dir, output, storage, stats)?;

                batch.push((object_hash_id, Arc::from(output.as_slice())));
            }
        }
        Object::Blob(blob_id) => {
            debug_assert!(!blob_id.is_inline());

            let blob = storage.get_blob(*blob_id)?;
            output.write_all(&[ID_BLOB])?;
            output.write_all(blob.as_ref())?;

            stats.add_blob(blob.len());

            batch.push((object_hash_id, Arc::from(output.as_slice())));
        }
        Object::Commit(commit) => {
            output.write_all(&[ID_COMMIT])?;

            let parent_hash_id = commit.parent_commit_hash.map(|h| h.as_u32()).unwrap_or(0);
            serialize_hash_id(parent_hash_id, output)?;

            let root_hash_id = commit.root_hash.as_u32();
            serialize_hash_id(root_hash_id, output)?;

            output.write_all(&commit.time.to_ne_bytes())?;

            let author_length: u32 = commit.author.len().try_into()?;
            output.write_all(&author_length.to_ne_bytes())?;
            output.write_all(commit.author.as_bytes())?;

            // The message length is inferred.
            // It's until the end of the slice
            output.write_all(commit.message.as_bytes())?;

            batch.push((object_hash_id, Arc::from(output.as_slice())));
        }
    }

    stats.total_bytes += output.len();

    Ok(())
}

fn serialize_hash_id(hash_id: u32, output: &mut Vec<u8>) -> Result<usize, SerializationError> {
    if hash_id & FULL_23_BITS == hash_id {
        // The HashId fits in 23 bits

        // Set `COMPACT_HASH_ID_BIT` so the deserializer knows the `HashId` is in 3 bytes
        let hash_id: u32 = hash_id | COMPACT_HASH_ID_BIT;
        let hash_id: [u8; 4] = hash_id.to_be_bytes();

        output.write_all(&hash_id[1..])?;
        Ok(3)
    } else if hash_id & FULL_31_BITS == hash_id {
        // HashId fits in 31 bits

        output.write_all(&hash_id.to_be_bytes())?;
        Ok(4)
    } else {
        // The HashId must not be 32 bits because we use the
        // MSB to determine if the HashId is compact or not
        Err(SerializationError::HashIdTooBig)
    }
}

/// Describes which pointers are set and at what index.
///
/// `Inode::Pointers` is an array of 32 pointers.
/// Each pointer is either set (`Some`) or not set (`None`).
///
/// Example:
/// Let's say that there are 2 pointers sets in the array, at the index
/// 1 and 7.
/// This would be represented in this bitfield as:
/// `0b01000001_00000000_00000000`
///
#[derive(Copy, Clone, Default, Debug)]
struct PointersDescriptor {
    bitfield: u32,
}

impl PointersDescriptor {
    /// Set bit at index in the bitfield
    fn set(&mut self, index: usize) {
        self.bitfield |= 1 << index;
    }

    /// Get bit at index in the bitfield
    fn get(&self, index: usize) -> bool {
        self.bitfield & 1 << index != 0
    }

    fn to_bytes(&self) -> [u8; 4] {
        self.bitfield.to_ne_bytes()
    }

    /// Iterates on all the bit sets in the bitfield.
    ///
    /// The iterator returns the index of the bit.
    fn iter(&self) -> PointersDescriptorIterator {
        PointersDescriptorIterator {
            bitfield: *self,
            current: 0,
        }
    }

    fn from_bytes(bytes: [u8; 4]) -> Self {
        Self {
            bitfield: u32::from_ne_bytes(bytes),
        }
    }

    /// Count number of bit set in the bitfield.
    fn count(&self) -> u8 {
        self.bitfield.count_ones() as u8
    }
}

impl From<&[Option<PointerToInode>; 32]> for PointersDescriptor {
    fn from(pointers: &[Option<PointerToInode>; 32]) -> Self {
        let mut bitfield = Self::default();

        for (index, pointer) in pointers.iter().enumerate() {
            if pointer.is_some() {
                bitfield.set(index);
            }
        }

        bitfield
    }
}

/// Iterates on all the bit sets in the bitfield.
///
/// The iterator returns the index of the bit.
struct PointersDescriptorIterator {
    bitfield: PointersDescriptor,
    current: usize,
}

impl Iterator for PointersDescriptorIterator {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        for index in self.current..32 {
            if self.bitfield.get(index) {
                self.current = index + 1;
                return Some(index);
            }
        }

        None
    }
}

fn serialize_inode(
    inode_id: InodeId,
    output: &mut Vec<u8>,
    hash_id: HashId,
    storage: &Storage,
    stats: &mut SerializeStats,
    batch: &mut Vec<(HashId, Arc<[u8]>)>,
    referenced_older_entries: &mut Vec<HashId>,
) -> Result<(), SerializationError> {
    use SerializationError::*;

    output.clear();
    let inode = storage.get_inode(inode_id)?;

    match inode {
        Inode::Pointers {
            depth,
            nchildren,
            npointers: _,
            pointers,
        } => {
            output.write_all(&[ID_INODE_POINTERS])?;
            output.write_all(&depth.to_ne_bytes())?;
            output.write_all(&nchildren.to_ne_bytes())?;

            let bitfield = PointersDescriptor::from(pointers);
            output.write_all(&bitfield.to_bytes())?;

            // Make sure that INODE_POINTERS_NBYTES_TO_HASHES is correct.
            debug_assert_eq!(output.len(), INODE_POINTERS_NBYTES_TO_HASHES);

            for pointer in pointers.iter().filter_map(|p| p.as_ref()) {
                let hash_id = pointer.hash_id().ok_or(MissingHashId)?;
                let hash_id = hash_id.as_u32();

                serialize_hash_id(hash_id, output)?;
            }

            batch.push((hash_id, Arc::from(output.as_slice())));

            // Recursively serialize all children
            for pointer in pointers.iter().filter_map(|p| p.as_ref()) {
                let hash_id = pointer.hash_id().ok_or(MissingHashId)?;

                if pointer.is_commited() {
                    // We only want to serialize new inodes.
                    // We skip inodes that were previously serialized and already
                    // in the repository.
                    // Add their hash_id to `referenced_older_entries` so the gargage
                    // collector won't collect them.
                    referenced_older_entries.push(hash_id);
                    continue;
                }

                let inode_id = pointer.inode_id();
                serialize_inode(
                    inode_id,
                    output,
                    hash_id,
                    storage,
                    stats,
                    batch,
                    referenced_older_entries,
                )?;
            }
        }
        Inode::Directory(dir_id) => {
            // We don't check if it's a new inode because the parent
            // caller (recursively) confirmed it's a new one.

            output.write_all(&[ID_INODE_DIRECTORY])?;
            let dir = storage.get_small_dir(*dir_id)?;
            serialize_directory(dir, output, storage, stats)?;

            batch.push((hash_id, Arc::from(output.as_slice())));
        }
    };

    Ok(())
}

#[derive(Debug)]
pub enum DeserializationError {
    UnexpectedEOF,
    TryFromSliceError,
    Utf8Error,
    UnknownID,
    FromUtf8Error,
    MissingRootHash,
    MissingHash,
    NodeIdError,
    StorageIdError { error: StorageError },
    InodeNotFoundInRepository,
    InodeEmptyInRepository,
}

impl From<TryFromSliceError> for DeserializationError {
    fn from(_: TryFromSliceError) -> Self {
        Self::TryFromSliceError
    }
}

impl From<Utf8Error> for DeserializationError {
    fn from(_: Utf8Error) -> Self {
        Self::Utf8Error
    }
}

impl From<FromUtf8Error> for DeserializationError {
    fn from(_: FromUtf8Error) -> Self {
        Self::FromUtf8Error
    }
}

impl From<NodeIdError> for DeserializationError {
    fn from(_: NodeIdError) -> Self {
        Self::NodeIdError
    }
}

impl From<StorageError> for DeserializationError {
    fn from(error: StorageError) -> Self {
        Self::StorageIdError { error }
    }
}

fn deserialize_hash_id(data: &[u8]) -> Result<(Option<HashId>, usize), DeserializationError> {
    use DeserializationError::*;

    let byte_hash_id = data.get(0).copied().ok_or(UnexpectedEOF)?;

    if byte_hash_id & 1 << 7 != 0 {
        // The HashId is in 3 bytes
        let hash_id = data.get(0..3).ok_or(UnexpectedEOF)?;

        let hash_id: u32 = (hash_id[0] as u32) << 16 | (hash_id[1] as u32) << 8 | hash_id[2] as u32;

        // Clear `COMPACT_HASH_ID_BIT`
        let hash_id = hash_id & (COMPACT_HASH_ID_BIT - 1);
        let hash_id = HashId::new(hash_id);

        Ok((hash_id, 3))
    } else {
        // The HashId is in 4 bytes
        let hash_id = data.get(0..4).ok_or(UnexpectedEOF)?;
        let hash_id = u32::from_be_bytes(hash_id.try_into()?);
        let hash_id = HashId::new(hash_id);

        Ok((hash_id, 4))
    }
}

fn deserialize_directory(
    data: &[u8],
    storage: &mut Storage,
) -> Result<DirectoryId, DeserializationError> {
    use DeserializationError as Error;
    use DeserializationError::*;

    let mut pos = 1;
    let data_length = data.len();

    let dir_id = storage.with_new_dir::<_, Result<_, Error>>(|storage, new_dir| {
        while pos < data_length {
            let descriptor = data.get(pos..pos + 1).ok_or(UnexpectedEOF)?;
            let descriptor = KeyNodeDescriptor::from_bytes([descriptor[0]; 1]);

            pos += 1;

            let key_id = match descriptor.key_inline_length() as usize {
                len if len > 0 => {
                    // The key is in the next `len` bytes
                    let key_bytes = data.get(pos..pos + len).ok_or(UnexpectedEOF)?;
                    let key_str = std::str::from_utf8(key_bytes)?;
                    pos += len;
                    storage.get_string_id(key_str)
                }
                _ => {
                    // The key length is in 2 bytes, followed by the key itself
                    let key_length = data.get(pos..pos + 2).ok_or(UnexpectedEOF)?;
                    let key_length = u16::from_ne_bytes(key_length.try_into()?);
                    let key_length = key_length as usize;

                    let key_bytes = data
                        .get(pos + 2..pos + 2 + key_length)
                        .ok_or(UnexpectedEOF)?;
                    let key_str = std::str::from_utf8(key_bytes)?;
                    pos += 2 + key_length;
                    storage.get_string_id(key_str)
                }
            };

            let kind = descriptor.kind();
            let blob_inline_length = descriptor.blob_inline_length() as usize;

            let node = if blob_inline_length > 0 {
                // The blob is inlined

                let blob = data
                    .get(pos..pos + blob_inline_length)
                    .ok_or(UnexpectedEOF)?;
                let blob_id = storage.add_blob_by_ref(blob)?;

                pos += blob_inline_length;

                Node::new_commited(kind, None, Some(Object::Blob(blob_id)))
            } else {
                let bytes = data.get(pos..).ok_or(UnexpectedEOF)?;
                let (hash_id, nbytes) = deserialize_hash_id(bytes)?;

                pos += nbytes;

                Node::new_commited(kind, Some(hash_id.ok_or(MissingHash)?), None)
            };

            let node_id = storage.add_node(node)?;

            new_dir.push((key_id, node_id));
        }

        Ok(storage.append_to_directories(new_dir))
    })??;

    Ok(dir_id)
}

/// Extract values from `data` to store them in `storage`.
/// Return an `Object`, which can be ids (refering to data inside `storage`) or a `Commit`
pub fn deserialize(
    data: &[u8],
    storage: &mut Storage,
    store: &ContextKeyValueStore,
) -> Result<Object, DeserializationError> {
    use DeserializationError::*;

    let mut pos = 1;

    match data.get(0).copied().ok_or(UnexpectedEOF)? {
        ID_DIRECTORY => {
            let dir_id = deserialize_directory(data, storage)?;
            Ok(Object::Directory(dir_id))
        }
        ID_BLOB => {
            let blob = data.get(pos..).ok_or(UnexpectedEOF)?;
            let blob_id = storage.add_blob_by_ref(blob)?;
            Ok(Object::Blob(blob_id))
        }
        ID_COMMIT => {
            let bytes = data.get(pos..).ok_or(UnexpectedEOF)?;
            let (parent_commit_hash, nbytes) = deserialize_hash_id(bytes)?;

            pos += nbytes;

            let bytes = data.get(pos..).ok_or(UnexpectedEOF)?;
            let (root_hash, nbytes) = deserialize_hash_id(bytes)?;

            pos += nbytes;

            let time = data.get(pos..pos + 8).ok_or(UnexpectedEOF)?;
            let time = u64::from_ne_bytes(time.try_into()?);

            let author_length = data.get(pos + 8..pos + 12).ok_or(UnexpectedEOF)?;
            let author_length = u32::from_ne_bytes(author_length.try_into()?) as usize;

            let author = data
                .get(pos + 12..pos + 12 + author_length)
                .ok_or(UnexpectedEOF)?;
            let author = author.to_vec();

            pos = pos + 12 + author_length;

            let message = data.get(pos..).ok_or(UnexpectedEOF)?;
            let message = message.to_vec();

            Ok(Object::Commit(Box::new(Commit {
                parent_commit_hash,
                root_hash: root_hash.ok_or(MissingRootHash)?,
                time,
                author: String::from_utf8(author)?,
                message: String::from_utf8(message)?,
            })))
        }
        ID_INODE_POINTERS => {
            let inode = deserialize_inode_pointers(&data[1..], storage, store)?;
            let inode_id = storage.add_inode(inode)?;

            Ok(Object::Directory(inode_id.into()))
        }
        _ => Err(UnknownID),
    }
}

fn deserialize_inode_pointers(
    data: &[u8],
    storage: &mut Storage,
    store: &ContextKeyValueStore,
) -> Result<Inode, DeserializationError> {
    use DeserializationError::*;

    let mut pos = 0;

    let depth = data.get(pos..pos + 4).ok_or(UnexpectedEOF)?;
    let depth = u32::from_ne_bytes(depth.try_into()?);

    let nchildren = data.get(pos + 4..pos + 8).ok_or(UnexpectedEOF)?;
    let nchildren = u32::from_ne_bytes(nchildren.try_into()?);

    pos += 8;

    let descriptor = data.get(pos..pos + 4).ok_or(UnexpectedEOF)?;
    let descriptor = PointersDescriptor::from_bytes(descriptor.try_into()?);

    let npointers = descriptor.count();
    let indexes_iter = descriptor.iter();

    pos += 4;

    let mut pointers: [Option<PointerToInode>; 32] = Default::default();

    for index in indexes_iter {
        let bytes = data.get(pos..).ok_or(UnexpectedEOF)?;
        let (hash_id, nbytes) = deserialize_hash_id(bytes)?;

        pos += nbytes;

        let inode_id = store
            .get_value(hash_id.ok_or(MissingHash)?)
            .map_err(|_| InodeNotFoundInRepository)
            .and_then(|data| {
                let data = data.ok_or(InodeEmptyInRepository)?;
                deserialize_inode(data.as_ref(), storage, store)
            })?;

        pointers[index as usize] = Some(PointerToInode::new_commited(
            Some(hash_id.ok_or(MissingHash)?),
            inode_id,
        ));
    }

    Ok(Inode::Pointers {
        depth,
        nchildren,
        npointers,
        pointers,
    })
}

pub fn deserialize_inode(
    data: &[u8],
    storage: &mut Storage,
    store: &ContextKeyValueStore,
) -> Result<InodeId, DeserializationError> {
    use DeserializationError::*;

    match data.get(0).copied().ok_or(UnexpectedEOF)? {
        ID_INODE_POINTERS => {
            let inode = deserialize_inode_pointers(&data[1..], storage, store)?;
            storage.add_inode(inode).map_err(Into::into)
        }
        ID_INODE_DIRECTORY => {
            let dir_id = deserialize_directory(data, storage)?;
            storage
                .add_inode(Inode::Directory(dir_id))
                .map_err(Into::into)
        }
        _ => Err(UnknownID),
    }
}

/// Iterate HashIds in the serialized data
pub fn iter_hash_ids(data: &[u8]) -> HashIdIterator {
    HashIdIterator { data, pos: 0 }
}

pub struct HashIdIterator<'a> {
    data: &'a [u8],
    pos: usize,
}

/// Number of bytes to reach the hashes when serializing a `Inode::Pointers`.
///
/// This skip `ID_INODE_POINTERS`, `depth`, `nchildren` and `PointersDescriptor`.
const INODE_POINTERS_NBYTES_TO_HASHES: usize = 13;

impl<'a> Iterator for HashIdIterator<'a> {
    type Item = HashId;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.data.get(0).copied()?;

        loop {
            let mut pos = self.pos;

            if pos == 0 {
                if id == ID_BLOB {
                    // No HashId in Object::Blob
                    return None;
                } else if id == ID_COMMIT {
                    // Deserialize the parent hash to know it's size
                    let (_, nbytes) = deserialize_hash_id(self.data.get(1..)?).ok()?;

                    // Object::Commit.root_hash
                    let (root_hash, _) = deserialize_hash_id(self.data.get(1 + nbytes..)?).ok()?;
                    self.pos = self.data.len();

                    return root_hash;
                } else if id == ID_INODE_POINTERS {
                    // We skip the first bytes (ID_INODE_POINTERS, depth, nchildren, ..) to reach
                    // the hashes
                    pos += INODE_POINTERS_NBYTES_TO_HASHES;
                } else {
                    // ID_DIRECTORY or ID_INODE_DIRECTORY
                    debug_assert!([ID_DIRECTORY, ID_INODE_DIRECTORY].contains(&id));

                    // Skip the tag (ID_DIRECTORY or ID_INODE_DIRECTORY)
                    pos += 1;
                }
            }

            if id == ID_INODE_POINTERS {
                let bytes = self.data.get(pos..)?;
                let (hash_id, nbytes) = deserialize_hash_id(bytes).ok()?;

                self.pos = pos + nbytes;

                return hash_id;
            } else {
                // ID_DIRECTORY or ID_INODE_DIRECTORY

                let descriptor = self.data.get(pos..pos + 1)?;
                let descriptor = KeyNodeDescriptor::from_bytes([descriptor[0]; 1]);

                pos += 1;

                let offset = match descriptor.key_inline_length() as usize {
                    len if len > 0 => len,
                    _ => {
                        let key_length = self.data.get(pos..pos + 2)?;
                        let key_length = u16::from_ne_bytes(key_length.try_into().ok()?);
                        2 + key_length as usize
                    }
                };

                pos += offset;

                let blob_inline_length = descriptor.blob_inline_length() as usize;

                if blob_inline_length > 0 {
                    // No HashId when the blob is inlined, go to next object
                    self.pos = pos + blob_inline_length;
                    continue;
                }

                let bytes = self.data.get(pos..)?;
                let (hash_id, nbytes) = deserialize_hash_id(bytes).ok()?;

                self.pos = pos + nbytes;

                return hash_id;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use tezos_timing::SerializeStats;

    use crate::{
        hash::hash_object, kv_store::in_memory::InMemory, working_tree::storage::DirectoryId,
    };

    use super::*;

    #[test]
    fn test_serialize() {
        let mut storage = Storage::new();
        let mut repo = InMemory::try_new().unwrap();
        let mut stats = SerializeStats::default();
        let mut batch = Vec::new();
        let mut older_entries = Vec::new();
        let fake_hash_id = HashId::try_from(1).unwrap();

        // Test Object::Directory

        let dir_id = DirectoryId::empty();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "a",
                Node::new_commited(NodeKind::Leaf, HashId::new(1), None),
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "bab",
                Node::new_commited(NodeKind::Leaf, HashId::new(2), None),
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                Node::new_commited(NodeKind::Leaf, HashId::new(3), None),
            )
            .unwrap();

        let mut data = Vec::with_capacity(1024);
        serialize_object(
            &Object::Directory(dir_id),
            fake_hash_id,
            &mut data,
            &storage,
            &mut stats,
            &mut batch,
            &mut older_entries,
        )
        .unwrap();

        let object = deserialize(&data, &mut storage, &repo).unwrap();

        if let Object::Directory(object) = object {
            assert_eq!(
                storage.get_owned_dir(dir_id).unwrap(),
                storage.get_owned_dir(object).unwrap()
            )
        } else {
            panic!();
        }

        let iter = iter_hash_ids(&data);
        assert_eq!(iter.map(|h| h.as_u32()).collect::<Vec<_>>(), &[3, 1, 2]);

        // Test Object::Blob

        // Not inlined value
        let blob_id = storage.add_blob_by_ref(&[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();

        let mut data = Vec::with_capacity(1024);
        serialize_object(
            &Object::Blob(blob_id),
            fake_hash_id,
            &mut data,
            &storage,
            &mut stats,
            &mut batch,
            &mut older_entries,
        )
        .unwrap();
        let object = deserialize(&data, &mut storage, &repo).unwrap();
        if let Object::Blob(object) = object {
            let blob = storage.get_blob(object).unwrap();
            assert_eq!(blob.as_ref(), &[1, 2, 3, 4, 5, 6, 7, 8]);
        } else {
            panic!();
        }
        let iter = iter_hash_ids(&data);
        assert_eq!(iter.count(), 0);

        // Test Object::Commit

        let mut data = Vec::with_capacity(1024);

        let commit = Commit {
            parent_commit_hash: HashId::new(9876),
            root_hash: HashId::new(12345).unwrap(),
            time: 12345,
            author: "123".to_string(),
            message: "abc".to_string(),
        };

        serialize_object(
            &Object::Commit(Box::new(commit.clone())),
            fake_hash_id,
            &mut data,
            &storage,
            &mut stats,
            &mut batch,
            &mut older_entries,
        )
        .unwrap();
        let object = deserialize(&data, &mut storage, &repo).unwrap();
        if let Object::Commit(object) = object {
            assert_eq!(*object, commit);
        } else {
            panic!();
        }

        let iter = iter_hash_ids(&data);
        assert_eq!(iter.map(|h| h.as_u32()).collect::<Vec<_>>(), &[12345]);

        // Test Inode::Directory

        let mut pointers: [Option<PointerToInode>; 32] = Default::default();

        for index in 0..pointers.len() {
            let inode_value = Inode::Directory(DirectoryId::empty());
            let inode_value_id = storage.add_inode(inode_value).unwrap();

            let hash_id = HashId::new((index + 1) as u32).unwrap();

            repo.write_batch(vec![(hash_id, Arc::new([ID_INODE_DIRECTORY]))])
                .unwrap();

            pointers[index] = Some(PointerToInode::new(Some(hash_id), inode_value_id));
        }

        let inode = Inode::Pointers {
            depth: 100,
            nchildren: 200,
            npointers: 250,
            pointers,
        };

        let inode_id = storage.add_inode(inode).unwrap();

        let hash_id = HashId::new(123).unwrap();
        batch.clear();
        serialize_inode(
            inode_id,
            &mut data,
            hash_id,
            &storage,
            &mut stats,
            &mut batch,
            &mut older_entries,
        )
        .unwrap();

        let new_inode_id = deserialize_inode(&batch[0].1, &mut storage, &repo).unwrap();
        let new_inode = storage.get_inode(new_inode_id).unwrap();

        if let Inode::Pointers {
            depth,
            nchildren,
            npointers,
            pointers,
        } = new_inode
        {
            assert_eq!(*depth, 100);
            assert_eq!(*nchildren, 200);
            assert_eq!(*npointers, 32);

            for (index, pointer) in pointers.iter().enumerate() {
                let pointer = pointer.as_ref().unwrap();
                let hash_id = pointer.hash_id().unwrap();
                assert_eq!(hash_id.as_u32() as usize, index + 1);

                let inode = storage.get_inode(pointer.inode_id()).unwrap();
                match inode {
                    Inode::Directory(dir_id) => assert!(dir_id.is_empty()),
                    _ => panic!(),
                }
            }
        } else {
            panic!()
        }

        let iter = iter_hash_ids(&batch[0].1);
        assert_eq!(
            iter.map(|h| h.as_u32()).collect::<Vec<_>>(),
            (1..33).collect::<Vec<_>>()
        );

        // Test Inode::Value

        let dir_id = DirectoryId::empty();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "a",
                Node::new_commited(NodeKind::Leaf, HashId::new(1), None),
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "bab",
                Node::new_commited(NodeKind::Leaf, HashId::new(2), None),
            )
            .unwrap();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                Node::new_commited(NodeKind::Leaf, HashId::new(3), None),
            )
            .unwrap();

        let inode = Inode::Directory(dir_id);
        let inode_id = storage.add_inode(inode).unwrap();

        batch.clear();
        serialize_inode(
            inode_id,
            &mut data,
            hash_id,
            &storage,
            &mut stats,
            &mut batch,
            &mut older_entries,
        )
        .unwrap();

        let new_inode_id = deserialize_inode(&batch[0].1, &mut storage, &repo).unwrap();
        let new_inode = storage.get_inode(new_inode_id).unwrap();

        if let Inode::Directory(new_dir_id) = new_inode {
            assert_eq!(
                storage.get_owned_dir(dir_id).unwrap(),
                storage.get_owned_dir(*new_dir_id).unwrap()
            )
        }

        let iter = iter_hash_ids(&batch[0].1);
        assert_eq!(iter.map(|h| h.as_u32()).collect::<Vec<_>>(), &[3, 1, 2]);
    }

    #[test]
    fn test_serialize_empty_blob() {
        let mut repo = InMemory::try_new().expect("failed to create context");
        let mut storage = Storage::new();
        let mut stats = SerializeStats::default();
        let mut batch = Vec::new();
        let mut older_entries = Vec::new();

        let fake_hash_id = HashId::try_from(1).unwrap();

        let blob_id = storage.add_blob_by_ref(&[]).unwrap();
        let blob = Object::Blob(blob_id);
        let blob_hash_id = hash_object(&blob, &mut repo, &storage).unwrap();

        assert!(blob_hash_id.is_some());

        let dir_id = DirectoryId::empty();
        let dir_id = storage
            .dir_insert(
                dir_id,
                "a",
                Node::new_commited(NodeKind::Leaf, blob_hash_id, None),
            )
            .unwrap();

        let mut data = Vec::with_capacity(1024);

        serialize_object(
            &Object::Directory(dir_id),
            fake_hash_id,
            &mut data,
            &storage,
            &mut stats,
            &mut batch,
            &mut older_entries,
        )
        .unwrap();

        let object = deserialize(&data, &mut storage, &repo).unwrap();

        if let Object::Directory(object) = object {
            assert_eq!(
                storage.get_owned_dir(dir_id).unwrap(),
                storage.get_owned_dir(object).unwrap()
            )
        } else {
            panic!();
        }
    }
}
