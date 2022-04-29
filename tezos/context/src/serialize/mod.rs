// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    array::TryFromSliceError, convert::TryInto, num::TryFromIntError, str::Utf8Error,
    string::FromUtf8Error,
};

use modular_bitfield::prelude::*;
use tezos_timing::SerializeStats;
use thiserror::Error;

use crate::{
    chunks::ChunkedVec,
    hash::HashingError,
    kv_store::{in_memory::BATCH_CHUNK_CAPACITY, inline_boxed_slice::InlinedBoxedSlice, HashId},
    persistent::DBError,
    working_tree::{
        shape::DirectoryShapeError,
        storage::{DirEntryIdError, Storage, StorageError},
        string_interner::StringInterner,
        working_tree::SerializeOutput,
        Object,
    },
    ContextKeyValueStore,
};

use self::persistent::AbsoluteOffset;

pub mod in_memory;
pub mod persistent;

const COMPACT_HASH_ID_BIT: u64 = 1 << 31;

const FULL_47_BITS: u64 = 0x7FFFFFFFFFFF;
const FULL_31_BITS: u64 = 0x7FFFFFFF;

pub type SerializeObjectSignature = fn(
    &Object,                                                                // object
    HashId,                                                                 // object_hash_id
    &mut SerializeOutput,                                                   // output
    &Storage,                                                               // storage
    &StringInterner,                                                        // strings
    &mut SerializeStats,                                                    // statistics
    &mut ChunkedVec<(HashId, InlinedBoxedSlice), { BATCH_CHUNK_CAPACITY }>, // batch
    &mut ContextKeyValueStore,                                              // repository
) -> Result<Option<AbsoluteOffset>, SerializationError>;

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum ObjectLength {
    OneByte,
    TwoBytes,
    FourBytes,
}

#[derive(BitfieldSpecifier)]
#[bits = 3]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum ObjectTag {
    Directory,
    Blob,
    Commit,
    InodePointers,
    ShapedDirectory,
}

#[bitfield(bits = 8)]
#[derive(Debug)]
pub struct ObjectHeader {
    #[allow(dead_code)] // `bitfield` generates unused method on this `tag`
    tag: ObjectTag,
    length: ObjectLength,
    is_persistent: bool,
    #[skip]
    _unused: B2,
}

impl ObjectHeader {
    pub fn get_length(&self) -> ObjectLength {
        self.length()
    }

    pub fn get_is_persistent(&self) -> bool {
        self.is_persistent()
    }
}

pub fn get_object_tag(bytes: &[u8]) -> ObjectTag {
    let header = bytes.get(0).copied().unwrap();
    let header: ObjectHeader = ObjectHeader::from_bytes([header]);

    header.tag_or_err().unwrap()
}

#[derive(Debug, Error)]
pub enum SerializationError {
    #[error("IOError {error}")]
    IOError {
        #[from]
        error: std::io::Error,
    },
    #[error("Directory not found")]
    DirNotFound,
    #[error("Directory entry not found")]
    DirEntryNotFound,
    #[error("Blob not found")]
    BlobNotFound,
    #[error("Conversion from int failed: {error}")]
    TryFromIntError {
        #[from]
        error: TryFromIntError,
    },
    #[error("StorageIdError: {error}")]
    StorageIdError {
        #[from]
        error: StorageError,
    },
    #[error("HashId too big")]
    HashIdTooBig,
    #[error("Missing HashId")]
    MissingHashId,
    #[error("DBError: {error}")]
    DBError {
        #[from]
        error: DBError,
    },
    #[error("Missing Offset")]
    MissingOffset,
    #[error("Hashing Error: {error}")]
    HashingError {
        #[from]
        error: HashingError,
    },
    #[error("InodeId is missing")]
    MissingInodeId,
}

#[derive(Debug, Error)]
pub enum DeserializationError {
    #[error("Unexpected end of file")]
    UnexpectedEOF,
    #[error("Conversion from slice to an array failed")]
    TryFromSliceError {
        #[from]
        error: TryFromSliceError,
    },
    #[error("Bytes are not valid utf-8: {error}")]
    Utf8Error {
        #[from]
        error: Utf8Error,
    },
    #[error("UnknownID")]
    UnknownID,
    #[error("Vector is not valid utf-8: {error}")]
    FromUtf8Error {
        #[from]
        error: FromUtf8Error,
    },
    #[error("Root hash is missing")]
    MissingRootHash,
    #[error("Hash is missing")]
    MissingHash,
    #[error("Offset is missing")]
    MissingOffset,
    #[error("DirEntryIdError: {error}")]
    DirEntryIdError {
        #[from]
        error: DirEntryIdError,
    },
    #[error("StorageIdError: {error:?}")]
    StorageIdError {
        #[from]
        error: StorageError,
    },
    #[error("Inode not found in repository")]
    InodeNotFoundInRepository,
    #[error("Inode empty in repository")]
    InodeEmptyInRepository,
    #[error("DBError: {error:?}")]
    DBError {
        #[from]
        error: Box<DBError>,
    },
    #[error("Cannot find next shape")]
    CannotFindNextShape,
    #[error("Directory shape error: {error:?}")]
    DirectoryShapeError {
        #[from]
        error: DirectoryShapeError,
    },
    #[error("IOError: {error:?}")]
    IOError {
        #[from]
        error: std::io::Error,
    },
}

pub fn deserialize_hash_id(data: &[u8]) -> Result<(Option<HashId>, usize), DeserializationError> {
    use DeserializationError::*;

    let byte_hash_id = data.get(0).copied().ok_or(UnexpectedEOF)?;

    if byte_hash_id & 1 << 7 != 0 {
        // The HashId is in 4 bytes
        let hash_id = data.get(0..4).ok_or(UnexpectedEOF)?;

        let hash_id = u32::from_be_bytes(hash_id.try_into()?);
        let hash_id = hash_id as u64;

        // Clear `COMPACT_HASH_ID_BIT`
        let hash_id = hash_id & (COMPACT_HASH_ID_BIT - 1);
        let hash_id = HashId::new(hash_id);

        Ok((hash_id, 4))
    } else {
        // The HashId is in 6 bytes
        let hash_id = data.get(0..6).ok_or(UnexpectedEOF)?;

        let hash_id = (hash_id[0] as u64) << 40
            | (hash_id[1] as u64) << 32
            | (hash_id[2] as u64) << 24
            | (hash_id[3] as u64) << 16
            | (hash_id[4] as u64) << 8
            | (hash_id[5] as u64);

        let hash_id = HashId::new(hash_id);

        Ok((hash_id, 6))
    }
}

pub fn serialize_hash_id_impl(
    hash_id: Option<HashId>,
    output: &mut SerializeOutput,
    repository: &mut ContextKeyValueStore,
    stats: &mut SerializeStats,
) -> Result<(), SerializationError> {
    let hash_id = match hash_id {
        Some(hash_id) => repository.make_hash_id_ready_for_commit(hash_id)?.as_u64(),
        None => 0,
    };

    stats.highest_hash_id = stats.highest_hash_id.max(hash_id);

    if hash_id & FULL_31_BITS == hash_id {
        // The HashId fits in 31 bits

        // Set `COMPACT_HASH_ID_BIT` so the deserializer knows the `HashId` is in 4 bytes
        let hash_id: u64 = hash_id | COMPACT_HASH_ID_BIT;
        let hash_id: [u8; 8] = hash_id.to_be_bytes();

        output.write_all(&hash_id[4..])?;
        stats.hash_ids_length = stats.hash_ids_length.saturating_add(4);

        Ok(())
    } else if hash_id & FULL_47_BITS == hash_id {
        // HashId fits in 47 bits

        output.write_all(&hash_id.to_be_bytes()[2..])?;
        stats.hash_ids_length = stats.hash_ids_length.saturating_add(6);

        Ok(())
    } else {
        // The HashId must not be 48 bits because we use the
        // MSB to determine if the HashId is compact or not
        Err(SerializationError::HashIdTooBig)
    }
}

pub fn serialize_hash_id<T>(
    hash_id: T,
    output: &mut SerializeOutput,
    repository: &mut ContextKeyValueStore,
    stats: &mut SerializeStats,
) -> Result<(), SerializationError>
where
    T: Into<Option<HashId>>,
{
    let hash_id: Option<HashId> = hash_id.into();
    serialize_hash_id_impl(hash_id, output, repository, stats)
}
