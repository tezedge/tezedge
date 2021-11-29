// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    borrow::Cow, collections::hash_map::DefaultHasher, convert::TryInto, hash::Hasher, io::Write,
};

#[cfg(test)]
use std::sync::Arc;

use crypto::hash::ContextHash;
use tezos_timing::{RepositoryMemoryUsage, SerializeStats};

use crate::{
    gc::NotGarbageCollected,
    hash::OBJECT_HASH_LEN,
    initializer::IndexInitializationError,
    persistent::{
        file::{get_persistent_base_path, File, FileType},
        get_commit_hash,
        lock::Lock,
        DBError, Flushable, KeyValueStoreBackend, Persistable,
    },
    serialize::{
        deserialize_hash_id,
        persistent::{self, read_object_length, AbsoluteOffset},
        DeserializationError, ObjectHeader,
    },
    working_tree::{
        shape::{DirectoryShapeId, DirectoryShapes, ShapeStrings},
        storage::{DirEntryId, Storage},
        string_interner::{StringId, StringInterner},
        working_tree::{PostCommitData, WorkingTree},
        Object, ObjectReference,
    },
    Map, ObjectHash,
};

use super::{hashes::HashesContainer, HashId, VacantObjectHash};

const FIRST_READ_OBJECT_LENGTH: usize = 4096;

pub struct Persistent {
    /// Concatenation of all objects
    ///
    /// See `serialize::persistent`
    data_file: File,
    /// Store shapes.
    ///
    /// File format:
    /// Concatenation of `StringId` (u32):
    /// [StringId(0), StringId(9), StringId(3), StringId(33), ..]
    ///
    shape_file: File,
    /// Store shapes indexes.
    ///
    /// File format:
    /// Concatenation of `ShapeSliceId` (u64):
    /// [ShapeSliceId { start: 0, end: 2 }, ShapeSliceId { start: 2, end: 10}, ..]
    ///
    /// In this example, and with the documentation of `shape_file`, the first
    /// shape (start: 0, end: 2) would be composed of the StringId `0`, `9` and `3`
    ///
    shape_index_file: File,
    /// Store commit indexes.
    ///
    /// File format:
    /// [[HashId (u32), offset (u64), context hash (32 bytes)], [..], ..]
    ///
    /// For each commit, we store its `HashId`, its offset in `data_file` and its
    /// context hash
    ///
    commit_index_file: File,
    /// Store small strings.
    ///
    /// File format:
    /// [[length (u8), string (nbytes)], [..], ..]
    ///
    /// The strings 'z' and 'abc' would be written as:
    /// [1, 'z', 3, 'a', 'b', 'c']
    ///
    strings_file: File,
    /// Store bigs strings
    ///
    /// File format:
    /// [[length (u32), string (nbytes)], [..], ..]
    ///
    big_strings_file: File,

    hashes: Hashes,
    shapes: DirectoryShapes,
    string_interner: StringInterner,

    pub context_hashes: Map<u64, ObjectReference>,

    // We keep the lock file here to invoke its destructor
    #[allow(dead_code)]
    lock_file: Lock,
}

impl NotGarbageCollected for Persistent {}

impl Flushable for Persistent {
    fn flush(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

impl Persistable for Persistent {
    fn is_persistent(&self) -> bool {
        true
    }
}

struct Hashes {
    /// List of hashes not yet commited
    in_memory: HashesContainer,
    /// Concatenation of all hashes commited
    ///
    /// [ObjectHash (32 bytes), ObjectHash (32 bytes), ..]
    ///
    hashes_file: File,
    /// Vector used to copy hashes from `Self::in_memory` and being able to
    /// write all hashes into the file in a single `File::append` call
    in_memory_bytes: Vec<u8>,
}

impl Hashes {
    fn try_new(hashes_file: File) -> Self {
        let hash_index = hashes_file.offset().as_u64() - hashes_file.start();

        debug_assert_eq!(hash_index as usize % OBJECT_HASH_LEN, 0);

        let in_memory_first_index = (hash_index as usize) / OBJECT_HASH_LEN;

        Self {
            in_memory: HashesContainer::new(in_memory_first_index),
            hashes_file,
            in_memory_bytes: Vec::with_capacity(1000),
        }
    }

    fn get_hash(&self, hash_id: HashId) -> Result<Cow<ObjectHash>, DBError> {
        if let Some(hash) = self.in_memory.try_get_hash(hash_id)? {
            // The hash is in memory

            Ok(Cow::Borrowed(hash))
        } else {
            // The hash is in the file

            let hash_id_index: usize = hash_id.try_into()?;
            let offset = hash_id_index * std::mem::size_of::<ObjectHash>();

            let offset = offset as u64 + self.hashes_file.start();

            let mut hash: ObjectHash = Default::default();

            self.hashes_file
                .read_exact_at(&mut hash, (offset as u64).into())?;

            Ok(Cow::Owned(hash))
        }
    }

    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        self.in_memory.get_vacant_object_hash()
    }

    fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        let hash_id: usize = hash_id.try_into()?;

        Ok(hash_id < self.in_memory.total_number_of_hashes())
    }

    fn commit(&mut self) -> Result<(), std::io::Error> {
        let in_memory = self.in_memory.get_commiting();

        if in_memory.is_empty() {
            self.in_memory.commited();
            return Ok(());
        }

        // Copy all hashes into the flat vector `Self::in_memory_bytes`
        self.in_memory_bytes.clear();
        for hash in in_memory {
            self.in_memory_bytes.extend_from_slice(hash);
        }

        self.hashes_file.append(&self.in_memory_bytes)?;

        self.in_memory.commited();

        Ok(())
    }
}

impl Persistent {
    pub fn try_new(db_path: Option<&str>) -> Result<Persistent, IndexInitializationError> {
        let base_path = get_persistent_base_path(db_path);

        let lock_file = Lock::try_lock(&base_path)?;

        let data_file = File::try_new(&base_path, FileType::Data)?;
        let mut shape_file = File::try_new(&base_path, FileType::ShapeDirectories)?;
        let mut shape_index_file = File::try_new(&base_path, FileType::ShapeDirectoriesIndex)?;
        let commit_index_file = File::try_new(&base_path, FileType::CommitIndex)?;
        let mut strings_file = File::try_new(&base_path, FileType::Strings)?;
        let mut big_strings_file = File::try_new(&base_path, FileType::BigStrings)?;

        let hashes_file = File::try_new(&base_path, FileType::Hashes)?;

        let shapes = DirectoryShapes::deserialize(&mut shape_file, &mut shape_index_file)?;
        let string_interner =
            StringInterner::deserialize(&mut strings_file, &mut big_strings_file)?;
        let (hashes, context_hashes) = deserialize_hashes(hashes_file, &commit_index_file)?;

        Ok(Self {
            data_file,
            shape_file,
            shape_index_file,
            commit_index_file,
            strings_file,
            hashes,
            big_strings_file,
            shapes,
            string_interner,
            context_hashes,
            lock_file,
        })
    }

    pub fn get_object_bytes<'a>(
        &self,
        object_ref: ObjectReference,
        buffer: &'a mut Vec<u8>,
    ) -> Result<&'a [u8], DBError> {
        let offset = object_ref.offset();

        if buffer.len() < FIRST_READ_OBJECT_LENGTH {
            buffer.resize(FIRST_READ_OBJECT_LENGTH, 0);
        }

        let buffer_length = buffer.len();

        // We attempt to read FIRST_READ_OBJECT_LENGTH bytes, if it's
        // not enough we will read more later
        let buffer_slice = self
            .data_file
            .read_at_most(&mut buffer[..FIRST_READ_OBJECT_LENGTH], offset)?;

        let object_header: ObjectHeader = ObjectHeader::from_bytes([buffer_slice[0]]);
        let (_, object_length) = read_object_length(buffer_slice, &object_header)?;

        if buffer_slice.len() < object_length {
            if buffer_length < object_length {
                buffer.resize(object_length, 0);
            }

            // Read the rest of the object
            self.data_file.read_exact_at(
                &mut buffer[FIRST_READ_OBJECT_LENGTH..object_length],
                offset.add_offset(FIRST_READ_OBJECT_LENGTH as u64),
            )?;
        }

        Ok(&buffer[..object_length])
    }

    fn get_hash_id_from_offset(&self, object_ref: ObjectReference) -> Result<HashId, DBError> {
        let offset = object_ref.offset();

        // We only need 10 bytes maximum to read the `HashId`
        let mut buffer: [u8; 10] = Default::default();

        self.data_file.read_at_most(&mut buffer, offset)?;

        let object_header: ObjectHeader = ObjectHeader::from_bytes([buffer[0]]);
        let (header_nbytes, _) = read_object_length(&buffer, &object_header)?;

        let (hash_id, _) = deserialize_hash_id(&buffer[header_nbytes..])?;

        Ok(hash_id.ok_or(DeserializationError::MissingHash)?)
    }

    fn commit_to_disk(&mut self, data: &[u8]) -> Result<(), std::io::Error> {
        self.data_file.append(data)?;

        let strings = self.string_interner.serialize();
        self.strings_file.append(&strings.strings)?;
        self.big_strings_file.append(&strings.big_strings)?;

        let shapes = self.shapes.serialize();
        self.shape_file.append(&shapes.shapes)?;
        self.shape_index_file.append(&shapes.index)?;

        self.hashes.commit()?;

        self.data_file.sync()?;
        self.strings_file.sync()?;
        self.big_strings_file.sync()?;
        self.hashes.hashes_file.sync()?;
        self.commit_index_file.sync()?;

        Ok(())
    }
}

fn deserialize_hashes(
    hashes_file: File,
    commit_index_file: &File,
) -> Result<(Hashes, Map<u64, ObjectReference>), DeserializationError> {
    let hashes = Hashes::try_new(hashes_file);
    let mut context_hashes: Map<u64, ObjectReference> = Default::default();

    let mut offset = commit_index_file.start();
    let end = commit_index_file.offset().as_u64();

    let mut hash_id_bytes = [0u8; 8];
    let mut hash_offset_bytes = [0u8; 8];
    let mut commit_hash: ObjectHash = Default::default();

    while offset < end {
        // commit index file is a sequence of entries that look like:
        // [hash_id 6 le bytes | offset u64 le bytes | hash <HASH_LEN> bytes]
        commit_index_file.read_exact_at(&mut hash_id_bytes[..6], offset.into())?;
        offset += (hash_id_bytes[..6]).len() as u64;
        let hash_id = u64::from_le_bytes(hash_id_bytes);

        commit_index_file.read_exact_at(&mut hash_offset_bytes, offset.into())?;
        offset += hash_offset_bytes.len() as u64;
        let hash_offset = u64::from_le_bytes(hash_offset_bytes);

        commit_index_file.read_exact_at(&mut commit_hash, offset.into())?;
        offset += commit_hash.len() as u64;

        let object_reference = ObjectReference::new(HashId::new(hash_id), Some(hash_offset.into()));

        let mut hasher = DefaultHasher::new();
        hasher.write(&commit_hash);
        let hashed = hasher.finish();

        context_hashes.insert(hashed, object_reference);
    }

    Ok((hashes, context_hashes))
}

fn serialize_context_hash(
    hash_id: HashId,
    offset: AbsoluteOffset,
    hash: &[u8],
) -> Result<Vec<u8>, DBError> {
    let mut output = Vec::<u8>::with_capacity(100);

    let offset: u64 = offset.as_u64();
    let hash_id: u64 = hash_id.as_u64();

    output.write_all(&hash_id.to_le_bytes()[..6])?;
    output.write_all(&offset.to_le_bytes())?;
    output.write_all(hash)?;

    debug_assert_eq!(hash.len(), OBJECT_HASH_LEN);

    Ok(output)
}

impl KeyValueStoreBackend for Persistent {
    fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        self.hashes.contains(hash_id)
    }

    fn put_context_hash(&mut self, object_ref: ObjectReference) -> Result<(), DBError> {
        let commit_hash = self.get_hash(object_ref)?;

        let mut hasher = DefaultHasher::new();
        hasher.write(&commit_hash[..]);
        let hashed = hasher.finish();

        let output = serialize_context_hash(
            object_ref.hash_id(),
            object_ref.offset(),
            commit_hash.as_ref(),
        )?;
        self.commit_index_file.append(&output)?;

        self.context_hashes.insert(hashed, object_ref);

        Ok(())
    }

    fn get_context_hash(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<ObjectReference>, DBError> {
        let mut hasher = DefaultHasher::new();
        hasher.write(context_hash.as_ref());
        let hashed = hasher.finish();

        Ok(self.context_hashes.get(&hashed).cloned())
    }

    fn get_hash(&self, object_ref: ObjectReference) -> Result<Cow<ObjectHash>, DBError> {
        let hash_id = self.get_hash_id(object_ref)?;

        self.hashes.get_hash(hash_id)
    }

    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        self.hashes.get_vacant_object_hash()
    }

    fn memory_usage(&self) -> RepositoryMemoryUsage {
        let strings_total_bytes = self.string_interner.memory_usage().total_bytes;
        let hashes_capacity = self.hashes.in_memory.total_capacity();
        let shapes_total_bytes = self.shapes.total_bytes();
        let commit_index_total_bytes = self.context_hashes.capacity()
            * (std::mem::size_of::<ObjectReference>() + std::mem::size_of::<u64>());

        RepositoryMemoryUsage {
            values_bytes: 0,
            values_capacity: 0,
            values_length: 0,
            hashes_capacity,
            hashes_length: 0,
            total_bytes: strings_total_bytes + hashes_capacity,
            npending_free_ids: 0,
            gc_npending_free_ids: 0,
            nshapes: self.shapes.nshapes(),
            strings_total_bytes,
            shapes_total_bytes,
            commit_index_total_bytes,
        }
    }

    fn get_shape(&self, shape_id: DirectoryShapeId) -> Result<ShapeStrings, DBError> {
        self.shapes
            .get_shape(shape_id)
            .map(ShapeStrings::SliceIds)
            .map_err(Into::into)
    }

    fn make_shape(
        &mut self,
        dir: &[(StringId, DirEntryId)],
    ) -> Result<Option<DirectoryShapeId>, DBError> {
        self.shapes.make_shape(dir).map_err(Into::into)
    }

    fn get_str(&self, string_id: StringId) -> Option<&str> {
        self.string_interner.get_str(string_id).ok()
    }

    fn synchronize_strings_from(&mut self, string_interner: &StringInterner) {
        self.string_interner.extend_from(string_interner);
    }

    fn get_object(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Object, DBError> {
        self.get_object_bytes(object_ref, &mut storage.data)?;

        let object_bytes = std::mem::take(&mut storage.data);
        let result = persistent::deserialize_object(
            &object_bytes,
            object_ref.offset(),
            storage,
            strings,
            self,
        );
        storage.data = object_bytes;

        result.map_err(Into::into)
    }

    fn get_object_bytes<'a>(
        &self,
        object_ref: ObjectReference,
        buffer: &'a mut Vec<u8>,
    ) -> Result<&'a [u8], DBError> {
        self.get_object_bytes(object_ref, buffer)
            .map_err(Into::into)
    }

    fn commit(
        &mut self,
        working_tree: &WorkingTree,
        parent_commit_ref: Option<ObjectReference>,
        author: String,
        message: String,
        date: u64,
    ) -> Result<(ContextHash, Box<SerializeStats>), DBError> {
        let offset = self.data_file.offset();

        self.hashes.in_memory.set_is_commiting();

        let PostCommitData {
            commit_ref,
            serialize_stats,
            output,
            ..
        } = working_tree
            .prepare_commit(
                date,
                author,
                message,
                parent_commit_ref,
                self,
                Some(persistent::serialize_object),
                Some(offset),
                false,
            )
            .map_err(Box::new)?;

        let commit_hash = get_commit_hash(commit_ref, self).map_err(Box::new)?;

        self.put_context_hash(commit_ref)?;

        self.commit_to_disk(&output)
            .map_err(|err| DBError::CommitToDiskError { err })?;

        Ok((commit_hash, serialize_stats))
    }

    fn get_hash_id(&self, object_ref: ObjectReference) -> Result<HashId, DBError> {
        match object_ref.hash_id_opt() {
            Some(hash_id) => Ok(hash_id),
            None => self.get_hash_id_from_offset(object_ref),
        }
    }

    fn take_strings_on_reload(&mut self) -> Option<StringInterner> {
        // On reload, `Self::string_interner` contains all strings and their hashes
        let string_interner = std::mem::take(&mut self.string_interner);

        // In the repository, we only want strings without their hashes
        self.synchronize_strings_from(&string_interner);

        self.string_interner
            .set_to_serialize_index(string_interner.get_to_serialize_index());

        Some(string_interner)
    }

    fn make_hash_id_ready_for_commit(&mut self, hash_id: HashId) -> Result<HashId, DBError> {
        self.hashes.in_memory.make_hash_id_ready_for_commit(hash_id)
    }

    #[cfg(test)]
    fn synchronize_data(
        &mut self,
        _batch: &[(HashId, Arc<[u8]>)],
        output: &[u8],
    ) -> Result<Option<AbsoluteOffset>, DBError> {
        self.commit_to_disk(output)?;
        self.hashes.in_memory.set_is_commiting();
        Ok(Some(self.data_file.offset()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Make sure that the commit index is correctly serialized & deserialized
    #[test]
    fn test_commit_index() {
        let hashes_file = File::try_new("test_commit_index", FileType::Hashes).unwrap();
        let mut commit_index_file =
            File::try_new("test_commit_index", FileType::CommitIndex).unwrap();

        let bytes =
            serialize_context_hash(HashId::new(101).unwrap(), 102.into(), &vec![3; 32]).unwrap();
        commit_index_file.append(bytes).unwrap();

        let bytes = serialize_context_hash(
            HashId::new(u32::MAX as u64).unwrap(),
            103.into(),
            &vec![4; 32],
        )
        .unwrap();
        commit_index_file.append(bytes).unwrap();

        let bytes = serialize_context_hash(
            HashId::new(u32::MAX as u64 + 10).unwrap(),
            104.into(),
            &vec![5; 32],
        )
        .unwrap();
        commit_index_file.append(bytes).unwrap();

        let res = deserialize_hashes(hashes_file, &commit_index_file).unwrap();

        let mut values: Vec<_> = res.1.values().collect();
        values.sort_by_key(|k| k.offset().as_u64());

        assert_eq!(
            &values,
            &[
                &ObjectReference::new(HashId::new(101), Some(102.into())),
                &ObjectReference::new(HashId::new(u32::MAX as u64), Some(103.into())),
                &ObjectReference::new(HashId::new(u32::MAX as u64 + 10), Some(104.into())),
            ]
        );

        std::fs::remove_file("test_commit_index/hashes.db").ok();
        std::fs::remove_file("test_commit_index/commit_index.db").ok();
    }
}
