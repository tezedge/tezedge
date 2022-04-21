// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module contains an implementation of Irmin's context hashes.
//!
//! A document describing the algorithm can be found [here](https://github.com/tarides/tezos-context-hash).

use std::{array::TryFromSliceError, io};

use blake2::digest::{InvalidOutputSize, Update, VariableOutput};
use blake2::VarBlake2b;
use thiserror::Error;

use ocaml::ocaml_hash_string;

use crate::kv_store::HashIdError;
use crate::working_tree::storage::DirectoryOrInodeId;
use crate::working_tree::string_interner::StringInterner;
use crate::working_tree::ObjectReference;
use crate::{
    kv_store::HashId,
    persistent::DBError,
    working_tree::{
        storage::{Blob, BlobId, DirectoryId, Inode, Storage, StorageError},
        Commit, DirEntryKind, Object,
    },
    ContextKeyValueStore,
};

mod ocaml;

pub const OBJECT_HASH_LEN: usize = 32;

pub type ObjectHash = [u8; OBJECT_HASH_LEN];

#[derive(Debug, Error)]
pub enum HashingError {
    #[error("Failed to encode LEB128 value: {error}")]
    Leb128EncodeFailure { error: io::Error },
    #[error("Invalid output size")]
    InvalidOutputSize,
    #[error("Failed to convert hash to array: {error}")]
    ConversionError { error: TryFromSliceError },
    #[error("Expected value instead of `None` for {0}")]
    ValueExpected(&'static str),
    #[error("Invalid hash value, reason: {0}")]
    InvalidHash(String),
    #[error("Missing Object")]
    MissingObject,
    #[error("The Object is borrowed more than once")]
    ObjectBorrow,
    #[error("Database error error {error:?}")]
    DBError { error: DBError },
    #[error("HashId not found: {hash_id:?}")]
    HashIdNotFound { hash_id: HashId },
    #[error("HashId conversion error: {error:?}")]
    HashIdError {
        #[from]
        error: HashIdError,
    },
    #[error("HashId empty")]
    HashIdEmpty,
    #[error("DirEntry not found")]
    DirEntryNotFound,
    #[error("Directory not found")]
    DirectoryNotFound,
    #[error("Blob not found")]
    BlobNotFound,
    #[error("StorageIdError: {error:?}")]
    StorageIdError { error: StorageError },
    #[error("Missing InodeId")]
    MissingInodeId,
    #[error("Missing Pointer")]
    MissingPointer,
}

impl From<DBError> for HashingError {
    fn from(error: DBError) -> Self {
        HashingError::DBError { error }
    }
}

impl From<InvalidOutputSize> for HashingError {
    fn from(_error: InvalidOutputSize) -> Self {
        Self::InvalidOutputSize
    }
}

impl From<TryFromSliceError> for HashingError {
    fn from(error: TryFromSliceError) -> Self {
        HashingError::ConversionError { error }
    }
}

impl From<io::Error> for HashingError {
    fn from(error: io::Error) -> Self {
        Self::Leb128EncodeFailure { error }
    }
}

impl From<StorageError> for HashingError {
    fn from(error: StorageError) -> Self {
        Self::StorageIdError { error }
    }
}

fn encode_irmin_dir_entry_kind(kind: &DirEntryKind) -> [u8; 8] {
    match kind {
        DirEntryKind::Directory => [0, 0, 0, 0, 0, 0, 0, 0],
        DirEntryKind::Blob => [255, 0, 0, 0, 0, 0, 0, 0],
    }
}

pub(crate) fn index(depth: u32, name: &str) -> u32 {
    ocaml_hash_string(depth, name.as_bytes()) % 32
}

fn hash_long_inode(
    ptr_id: DirectoryOrInodeId,
    store: &mut ContextKeyValueStore,
    storage: &Storage,
    strings: &StringInterner,
) -> Result<HashId, HashingError> {
    let mut hasher = VarBlake2b::new(OBJECT_HASH_LEN)?;

    match ptr_id {
        DirectoryOrInodeId::Directory(dir_id) => {
            // Inode value:
            //
            // |   1   |   1  |     n_1      |  ...  |      n_k      |
            // +-------+------+--------------+-------+---------------+
            // | \000  |  \n  | prehash(e_1) |  ...  | prehash(e_k)  |
            //
            // where n_i = len(prehash(e_i))

            let dir = storage.get_small_dir(dir_id)?;

            hasher.update(&[0u8]); // type tag
            hasher.update(&[dir.len() as u8]);

            // Inode value object:
            //
            // |   (LEB128)  |  len(name)   |   1    |   32   |
            // +-------------+--------------+--------+--------+
            // | \len(name)  |     name     |  kind  |  hash  |

            for (name, dir_entry_id) in dir.as_ref() {
                let name = strings.get_str(*name)?;

                leb128::write::unsigned(&mut hasher, name.len() as u64)?;
                hasher.update(name.as_bytes());

                // \000 for nodes, and \001 for contents.
                let dir_entry = storage.get_dir_entry(*dir_entry_id)?;
                match dir_entry.dir_entry_kind() {
                    DirEntryKind::Blob => hasher.update(&[1u8]),
                    DirEntryKind::Directory => hasher.update(&[0u8]),
                };

                if let Some(blob) = dir_entry.get_inlined_blob(storage) {
                    hasher.update(&hash_inlined_blob(blob)?);
                } else {
                    hasher.update(dir_entry.object_hash(store, storage, strings)?.as_ref());
                }
            }
        }
        DirectoryOrInodeId::Inode(inode_id) => {
            let Inode {
                depth,
                nchildren,
                pointers,
                ..
            } = storage.get_inode(inode_id)?;

            let npointers = pointers.npointers();

            // Inode directory:
            //
            // |   1    | (LEB128) |   (LEB128)    |    1   |  33  | ... |  33  |
            // +--------+----------+---------------+--------+------+-----+------+
            // |  \001  |  depth   | len(children) |   \k   | s_1  | ... | s_k  |

            hasher.update(&[1u8]); // type tag
            leb128::write::unsigned(&mut hasher, *depth as u64)?;
            leb128::write::unsigned(&mut hasher, *nchildren as u64)?;
            hasher.update(&[npointers as u8]);

            // Inode pointer:

            //
            // |    1    |   32   |
            // +---------+--------+
            // |  index  |  hash  |

            for (ptr_index, thin_pointer_id) in pointers.iter() {
                let pointer = storage.pointer_copy(thin_pointer_id)?;

                let ptr_index: u8 = ptr_index as u8;

                hasher.update(&[ptr_index]);

                let hash_id = match storage.pointer_retrieve_hashid(&pointer, store)? {
                    Some(hash_id) => hash_id,
                    None => {
                        let ptr_id = pointer.ptr_id().ok_or(HashingError::MissingInodeId)?;

                        let hash_id = hash_long_inode(ptr_id, store, storage, strings)?;
                        storage.pointer_set_hashid(&pointer, hash_id)?;

                        hash_id
                    }
                };

                let hash = store.get_hash(ObjectReference::new(Some(hash_id), None))?;

                hasher.update(hash.as_ref());
            }
        }
    }

    let hash_id = store
        .get_vacant_object_hash()?
        .write_with(|object| hasher.finalize_variable(|r| object.copy_from_slice(r)))?;

    Ok(hash_id)
}

// hash is calculated as:
// <number of child nodes (8 bytes)><CHILD NODE>
// where:
// - CHILD NODE - <NODE TYPE><length of string (1 byte)><string/path bytes><length of hash (8bytes)><hash bytes>
// - NODE TYPE - blob dir_entry(0xff0000000000000000) or internal dir_entry (0x0000000000000000)
fn hash_short_inode(
    dir_id: DirectoryId,
    store: &mut ContextKeyValueStore,
    storage: &Storage,
    strings: &StringInterner,
) -> Result<HashId, HashingError> {
    let mut hasher = VarBlake2b::new(OBJECT_HASH_LEN)?;

    // DirEntry list:
    //
    // |    8   |     n_1      | ... |      n_k     |
    // +--------+--------------+-----+--------------+
    // |   \k   | prehash(e_1) | ... | prehash(e_k) |

    let dir = storage.get_small_dir(dir_id)?;
    hasher.update(&(dir.len() as u64).to_be_bytes());

    // DirEntry object:
    //
    // |   8   |   (LEB128)   |  len(name)  |   8   |   32   |
    // +-------+--------------+-------------+-------+--------+
    // | kind  |  \len(name)  |    name     |  \32  |  hash  |

    for (k, v) in dir.as_ref() {
        let v = storage.get_dir_entry(*v)?;
        hasher.update(encode_irmin_dir_entry_kind(&v.dir_entry_kind()));
        // Key length is written in LEB128 encoding

        let k = strings.get_str(*k)?;
        leb128::write::unsigned(&mut hasher, k.len() as u64)?;
        hasher.update(k.as_bytes());
        hasher.update(&(OBJECT_HASH_LEN as u64).to_be_bytes());

        if let Some(blob) = v.get_inlined_blob(storage) {
            hasher.update(&hash_inlined_blob(blob)?);
        } else {
            hasher.update(v.object_hash(store, storage, strings)?.as_ref());
        }
    }

    let hash_id = store
        .get_vacant_object_hash()?
        .write_with(|object| hasher.finalize_variable(|r| object.copy_from_slice(r)))?;

    Ok(hash_id)
}

// Calculates hash of directory
// uses BLAKE2 binary 256 length hash function
pub(crate) fn hash_directory(
    dir_id: DirectoryId,
    store: &mut ContextKeyValueStore,
    storage: &Storage,
    strings: &StringInterner,
) -> Result<HashId, HashingError> {
    if let Some(inode_id) = dir_id.get_inode_id() {
        hash_long_inode(DirectoryOrInodeId::Inode(inode_id), store, storage, strings)
    } else {
        hash_short_inode(dir_id, store, storage, strings)
    }
}

// Calculates hash of BLOB
// uses BLAKE2 binary 256 length hash function
// hash is calculated as <length of data (8 bytes)><data>
pub(crate) fn hash_blob(
    blob_id: BlobId,
    store: &mut ContextKeyValueStore,
    storage: &Storage,
) -> Result<Option<HashId>, HashingError> {
    if blob_id.is_inline() {
        return Ok(None);
    }

    let mut hasher = VarBlake2b::new(OBJECT_HASH_LEN)?;

    let blob = storage.get_blob(blob_id)?;
    hasher.update(&(blob.len() as u64).to_be_bytes());
    hasher.update(blob);

    let hash_id = store
        .get_vacant_object_hash()?
        .write_with(|object| hasher.finalize_variable(|r| object.copy_from_slice(r)))?;

    Ok(Some(hash_id))
}

// Calculates hash of BLOB
// uses BLAKE2 binary 256 length hash function
// hash is calculated as <length of data (8 bytes)><data>
pub(crate) fn hash_inlined_blob(blob: Blob) -> Result<ObjectHash, HashingError> {
    let mut hasher = VarBlake2b::new(OBJECT_HASH_LEN)?;

    hasher.update(&(blob.len() as u64).to_be_bytes());
    hasher.update(blob);

    let mut object_hash: ObjectHash = Default::default();
    hasher.finalize_variable(|r| object_hash.copy_from_slice(r));

    Ok(object_hash)
}

// Calculates hash of commit
// uses BLAKE2 binary 256 length hash function
// hash is calculated as:
// <hash length (8 bytes)><directory hash bytes>
// <length of parent hash (8bytes)><parent hash bytes>
// <time in epoch format (8bytes)
// <commit author name length (8bytes)><commit author name bytes>
// <commit message length (8bytes)><commit message bytes>
pub(crate) fn hash_commit(
    commit: &Commit,
    store: &mut ContextKeyValueStore,
) -> Result<HashId, HashingError> {
    let mut hasher = VarBlake2b::new(OBJECT_HASH_LEN)?;
    hasher.update(&(OBJECT_HASH_LEN as u64).to_be_bytes());

    let root_hash = store.get_hash(commit.root_ref)?;
    hasher.update(root_hash.as_ref());

    if let Some(parent) = commit.parent_commit_ref {
        let parent_commit_hash = store.get_hash(parent)?;
        hasher.update(&(1_u64).to_be_bytes()); // # of parents; we support only 1
        hasher.update(&(parent_commit_hash.len() as u64).to_be_bytes());
        hasher.update(&parent_commit_hash.as_ref());
    } else {
        hasher.update(&(0_u64).to_be_bytes());
    }

    hasher.update(&(commit.time as u64).to_be_bytes());
    hasher.update(&(commit.author.len() as u64).to_be_bytes());
    hasher.update(&commit.author.clone().into_bytes());
    hasher.update(&(commit.message.len() as u64).to_be_bytes());
    hasher.update(&commit.message.clone().into_bytes());

    let hash_id = store
        .get_vacant_object_hash()?
        .write_with(|object| hasher.finalize_variable(|r| object.copy_from_slice(r)))?;

    Ok(hash_id)
}

pub(crate) fn hash_object(
    object: &Object,
    store: &mut ContextKeyValueStore,
    storage: &Storage,
    strings: &StringInterner,
) -> Result<Option<HashId>, HashingError> {
    match object {
        Object::Commit(commit) => hash_commit(commit, store).map(Some),
        Object::Directory(dir_id) => hash_directory(*dir_id, store, storage, strings).map(Some),
        Object::Blob(blob_id) => hash_blob(*blob_id, store, storage),
    }
}

#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    use std::{collections::HashSet, env, fs::File, io::Read, path::Path};

    use flate2::read::GzDecoder;

    use crypto::hash::{ContextHash, HashTrait};
    use tezos_timing::SerializeStats;

    use crate::chunks::ChunkedVec;
    use crate::kv_store::in_memory::{InMemory, InMemoryConfiguration, BATCH_CHUNK_CAPACITY};
    use crate::kv_store::inline_boxed_slice::InlinedBoxedSlice;
    use crate::kv_store::persistent::{Persistent, PersistentConfiguration};
    use crate::serialize::{in_memory, persistent, SerializeObjectSignature};
    use crate::working_tree::working_tree::SerializeOutput;
    use crate::working_tree::ObjectReference;
    use crate::working_tree::{DirEntry, DirEntryKind};

    use super::*;

    #[test]
    fn test_hash_of_commit_persistent() {
        let mut repo = Persistent::try_new(PersistentConfiguration {
            db_path: None,
            startup_check: true,
            read_mode: false,
        })
        .expect("failed to create persistent context");
        hash_of_commit(&mut repo);
    }

    #[test]
    fn test_hash_of_commit() {
        let mut repo = InMemory::try_new(InMemoryConfiguration {
            db_path: None,
            startup_check: true,
        })
        .expect("failed to create in-memory context");
        hash_of_commit(&mut repo);
    }

    fn hash_of_commit(repo: &mut ContextKeyValueStore) {
        // Calculates hash of commit
        // uses BLAKE2 binary 256 length hash function
        // hash is calculated as:
        // <hash length (8 bytes)><directory hash bytes>
        // <length of parent hash (8bytes)><parent hash bytes>
        // <time in epoch format (8bytes)
        // <commit author name length (8bytes)><commit author name bytes>
        // <commit message length (8bytes)><commit message bytes>
        let expected_commit_hash =
            "e6de3fd37b1dc2b3c9d072ea67c2c5be1b55eeed9f5377b2bfc1228e6f9cb69b";

        let hash = hex::decode("0d78b30e959c2a079e8ccb4ca19d428c95d29b2f02a35c1c58ef9c8972bc26aa")
            .unwrap();

        let hash_id = repo
            .get_vacant_object_hash()
            .unwrap()
            .write_with(|entry| entry.copy_from_slice(&hash))
            .unwrap();

        let dummy_commit = Commit {
            parent_commit_ref: None,
            root_ref: ObjectReference::new(Some(hash_id), Some(0.into())),
            time: 0,
            author: "Tezedge".to_string(),
            message: "persist changes".to_string(),
        };

        // hexademical representation of above commit:
        //
        // hash length (8 bytes)           ->  00 00 00 00 00 00 00 20
        // dir hash bytes                  ->  0d78b30e959c2a079e8ccb4ca19d428c95d29b2f02a35c1c58ef9c8972bc26aa
        // parents count                   ->  00 00 00 00 00 00 00 00  (0)
        // commit time                     ->  00 00 00 00 00 00 00 00  (0)
        // commit author name length       ->  00 00 00 00 00 00 00 07  (7)
        // commit author name ('Tezedge')  ->  54 65 7a 65 64 67 65     (Tezedge)
        // commit message length           ->  00 00 00 00 00 00 00 0xf (15)

        let mut bytes = String::new();
        let hash_length = "0000000000000020"; // 32
        let dir_hash = "0d78b30e959c2a079e8ccb4ca19d428c95d29b2f02a35c1c58ef9c8972bc26aa"; // dir hash bytes
        let parents_count = "0000000000000000"; // 0
        let commit_time = "0000000000000000"; // 0
        let commit_author_name_length = "0000000000000007"; // 7
        let commit_author_name = "54657a65646765"; // 'Tezedge'
        let commit_message_length = "000000000000000f"; // 15
        let commit_message = "70657273697374206368616e676573"; // 'persist changes'

        println!("calculating hash of commit: \n\t{:?}\n", dummy_commit);

        println!("[hex] hash_length : {}", hash_length);
        println!("[hex] dir_hash : {}", dir_hash);
        println!("[hex] parents_count : {}", parents_count);
        println!("[hex] commit_time : {}", commit_time);
        println!(
            "[hex] commit_author_name_length : {}",
            commit_author_name_length
        );
        println!("[hex] commit_author_name : {}", commit_author_name);
        println!("[hex] commit_message_length : {}", commit_message_length);
        println!("[hex] commit_message : {}", commit_message);

        bytes += hash_length;
        bytes += dir_hash;
        bytes += parents_count;
        bytes += commit_time;
        bytes += commit_author_name_length;
        bytes += commit_author_name;
        bytes += commit_message_length;
        bytes += commit_message;

        println!(
            "manually calculated haxedemical representation of commit: {}",
            bytes
        );

        let mut hasher = VarBlake2b::new(OBJECT_HASH_LEN).unwrap();
        hasher.update(hex::decode(bytes).unwrap());
        let calculated_commit_hash = hasher.finalize_boxed();

        println!(
            "calculated hash of the commit: {}",
            hex::encode(calculated_commit_hash.as_ref())
        );

        let hash_id = hash_commit(&dummy_commit, repo).unwrap();
        let object_ref = ObjectReference::new(Some(hash_id), None);

        assert_eq!(
            calculated_commit_hash.as_ref(),
            repo.get_hash(object_ref).unwrap().as_ref()
        );
        assert_eq!(
            expected_commit_hash,
            hex::encode(calculated_commit_hash.as_ref())
        );
    }

    #[test]
    fn test_hash_of_small_dir_persistent() {
        let mut repo = Persistent::try_new(PersistentConfiguration {
            db_path: None,
            startup_check: true,
            read_mode: false,
        })
        .expect("failed to create persistent context");
        hash_of_small_dir(&mut repo);
    }

    #[test]
    fn test_hash_of_small_dir() {
        let mut repo = InMemory::try_new(InMemoryConfiguration {
            db_path: None,
            startup_check: true,
        })
        .expect("failed to create in-memory context");
        hash_of_small_dir(&mut repo);
    }

    fn hash_of_small_dir(repo: &mut ContextKeyValueStore) {
        // Calculates hash of directory
        // uses BLAKE2 binary 256 length hash function
        // hash is calculated as:
        // <number of child nodes (8 bytes)><CHILD NODE>
        // where:
        // - CHILD NODE - <NODE TYPE><length of string (1 byte)><string/path bytes><length of hash (8bytes)><hash bytes>
        // - NODE TYPE - blob dir_entry(0xff00000000000000) or internal dir_entry (0x0000000000000000)
        let expected_dir_hash = "d49a53323107f2ae40b01eaa4e9bec4d02801daf60bab82dc2529e40d40fa917";
        let dummy_dir = DirectoryId::empty();

        let mut storage = Storage::new();
        let mut strings = StringInterner::default();

        let blob_id = storage.add_blob_by_ref(&[1]).unwrap();

        let dir_entry = DirEntry::new(DirEntryKind::Blob, Object::Blob(blob_id));

        let dummy_dir = storage
            .dir_insert(dummy_dir, "a", dir_entry, &mut strings, repo)
            .unwrap();

        // hexademical representation of above directory:
        //
        // number of child nodes           ->  00 00 00 00 00 00 00 01  (1)
        // dir_entry type                  ->  ff 00 00 00 00 00 00 00  (blob dir_entry)
        // length of string                ->  01                       (1)
        // string                          ->  61                       ('a')
        // length of hash                  ->  00 00 00 00 00 00 00 20  (32)
        // hash                            ->  407f958990678e2e9fb06758bc6520dae46d838d39948a4c51a5b19bd079293d

        let mut bytes = String::new();
        let child_dir_entries = "0000000000000001";
        let blob_dir_entry = "ff00000000000000";
        let string_length = "01";
        let string_value = "61";
        let hash_length = "0000000000000020";
        let hash = "407f958990678e2e9fb06758bc6520dae46d838d39948a4c51a5b19bd079293d";

        println!("calculating hash of directory: \n\t{:?}\n", dummy_dir);
        println!("[hex] child nodes count: {}", child_dir_entries);
        println!("[hex] blob_dir_entry   : {}", blob_dir_entry);
        println!("[hex] string_length    : {}", string_length);
        println!("[hex] string_value     : {}", string_value);
        println!("[hex] hash_length      : {}", hash_length);
        println!("[hex] hash             : {}", hash);

        bytes += child_dir_entries;
        bytes += blob_dir_entry;
        bytes += string_length;
        bytes += string_value;
        bytes += hash_length;
        bytes += hash;

        println!(
            "manually calculated haxedemical representation of directory: {}",
            bytes
        );

        let mut hasher = VarBlake2b::new(OBJECT_HASH_LEN).unwrap();
        hasher.update(hex::decode(bytes).unwrap());
        let calculated_dir_hash = hasher.finalize_boxed();

        println!(
            "calculated hash of the directory: {}",
            hex::encode(calculated_dir_hash.as_ref())
        );

        let hash_id = hash_directory(dummy_dir, repo, &storage, &strings).unwrap();
        let object_ref = ObjectReference::new(Some(hash_id), None);

        assert_eq!(
            calculated_dir_hash.as_ref(),
            repo.get_hash(object_ref).unwrap().as_ref(),
        );
        assert_eq!(
            calculated_dir_hash.as_ref(),
            hex::decode(expected_dir_hash).unwrap()
        );
    }

    // Tests from Tarides json dataset

    #[derive(serde::Deserialize)]
    struct DirEntryHashTest {
        hash: String,
        bindings: Vec<DirEntryHashBinding>,
    }

    #[derive(serde::Deserialize)]
    struct DirEntryHashBinding {
        name: String,
        kind: String,
        hash: String,
    }

    #[test]
    fn test_dir_entry_hashes_persistent() {
        let mut repo = Persistent::try_new(PersistentConfiguration {
            db_path: None,
            startup_check: true,
            read_mode: false,
        })
        .expect("failed to create persistent context");
        test_type_hashes("nodes.json.gz", &mut repo, persistent::serialize_object);
    }

    #[test]
    fn test_inode_hashes_persistent() {
        let mut repo = Persistent::try_new(PersistentConfiguration {
            db_path: None,
            startup_check: true,
            read_mode: false,
        })
        .expect("failed to create persistent context");
        test_type_hashes("inodes.json.gz", &mut repo, persistent::serialize_object);
    }

    #[test]
    fn test_dir_entry_hashes_memory() {
        let mut repo = InMemory::try_new(InMemoryConfiguration {
            db_path: None,
            startup_check: true,
        })
        .expect("failed to create in-memory context");
        test_type_hashes("nodes.json.gz", &mut repo, in_memory::serialize_object);
    }

    #[test]
    fn test_inode_hashes_memory() {
        let mut repo = InMemory::try_new(InMemoryConfiguration {
            db_path: None,
            startup_check: true,
        })
        .expect("failed to create in-memory context");
        test_type_hashes("inodes.json.gz", &mut repo, in_memory::serialize_object);
    }

    fn test_type_hashes(
        json_gz_file_name: &str,
        repo: &mut ContextKeyValueStore,
        serialize_fun: SerializeObjectSignature,
    ) {
        let offset = repo.synchronize_data(&[], &[]).unwrap();

        let mut json_file = open_hashes_json_gz(json_gz_file_name);
        let mut bytes = Vec::new();
        let mut output = SerializeOutput::new(offset);
        let mut strings = StringInterner::default();
        let mut stats = SerializeStats::default();

        // NOTE: reading from a stream is very slow with serde, thats why
        // the whole file is being read here before parsing.
        // See: https://github.com/serde-rs/json/issues/160#issuecomment-253446892
        json_file.read_to_end(&mut bytes).unwrap();

        let test_cases: Vec<DirEntryHashTest> = serde_json::from_slice(&bytes).unwrap();

        for test_case in test_cases {
            let mut storage = Storage::new();

            let bindings_count = test_case.bindings.len();
            let mut dir_id = DirectoryId::empty();
            let mut batch =
                ChunkedVec::<(HashId, InlinedBoxedSlice), BATCH_CHUNK_CAPACITY>::default();

            let mut names = HashSet::new();

            for binding in test_case.bindings {
                let dir_entry_kind = match binding.kind.as_str() {
                    "Tree" => DirEntryKind::Directory,
                    "Contents" => DirEntryKind::Blob,
                    other => panic!("Got unexpected binding kind: {}", other),
                };
                let object_hash = ContextHash::from_base58_check(&binding.hash).unwrap();

                let hash_id = repo
                    .get_vacant_object_hash()
                    .unwrap()
                    .write_with(|bytes| bytes.copy_from_slice(object_hash.as_ref()))
                    .unwrap();

                let object = match dir_entry_kind {
                    DirEntryKind::Blob => Object::Blob(
                        storage
                            .add_blob_by_ref(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
                            .unwrap(),
                    ),
                    DirEntryKind::Directory => Object::Directory(DirectoryId::empty()),
                };

                let dir_entry = DirEntry::new_commited(dir_entry_kind, Some(hash_id), Some(object));
                dir_entry.set_commited(false);

                names.insert(binding.name.clone());

                dir_id = storage
                    .dir_insert(dir_id, binding.name.as_str(), dir_entry, &mut strings, repo)
                    .unwrap();

                assert!(storage
                    .dir_find_dir_entry(dir_id, binding.name.as_str(), &mut strings, repo)
                    .unwrap()
                    .is_some());
            }

            // The following block insert and remove lots of nodes to make sure
            // that the implementation of `Storage::dir_insert` and `Storage::dir_remove`
            // is correct with Inodes
            {
                let hash_id = HashId::new(11111).unwrap();

                for index in 0..10000 {
                    let key = format!("abc{}", index);
                    dir_id = storage
                        .dir_insert(
                            dir_id,
                            &key,
                            DirEntry::new_commited(DirEntryKind::Blob, Some(hash_id), None)
                                .with_offset(1.into()),
                            &mut strings,
                            repo,
                        )
                        .unwrap();
                    let a = dir_id;

                    // Insert the same element (same key) twice, this must not increment
                    // `Inode::Pointers::nchildren`.
                    dir_id = storage
                        .dir_insert(
                            dir_id,
                            &key,
                            DirEntry::new_commited(DirEntryKind::Blob, Some(hash_id), None)
                                .with_offset(1.into()),
                            &mut strings,
                            repo,
                        )
                        .unwrap();
                    let b = dir_id;

                    assert_eq!(storage.dir_len(a).unwrap(), storage.dir_len(b).unwrap());
                }

                // Remove the elements we just inserted
                for index in 0..10000 {
                    let key = format!("abc{}", index);
                    dir_id = storage
                        .dir_remove(dir_id, &key, &mut strings, repo)
                        .unwrap();
                    let a = dir_id;

                    // Remove the same key twice
                    dir_id = storage
                        .dir_remove(dir_id, &key, &mut strings, repo)
                        .unwrap();
                    let b = dir_id;

                    // The 2nd remove should not modify the existing inode or create a new one
                    assert_eq!(a, b);
                }
            }

            let expected_hash = ContextHash::from_base58_check(&test_case.hash).unwrap();
            let computed_hash_id = hash_directory(dir_id, repo, &storage, &strings).unwrap();
            let computed_hash_ref = ObjectReference::new(Some(computed_hash_id), None);
            let computed_hash = repo.get_hash(computed_hash_ref).unwrap();
            let computed_hash = ContextHash::try_from_bytes(computed_hash.as_ref()).unwrap();

            // The following block makes sure that a serialized & deserialized inode
            // produce the same hash
            {
                output.clear();

                storage
                    .dir_iterate_unsorted(dir_id, |(_, dir_entry_id)| {
                        let dir_entry = storage.get_dir_entry(*dir_entry_id).unwrap();
                        let object = dir_entry.get_object().unwrap();
                        let object_hash_id = dir_entry.hash_id().unwrap();

                        repo.synchronize_data(&[], &[]).unwrap();

                        let offset = serialize_fun(
                            &object,
                            object_hash_id,
                            &mut output,
                            &storage,
                            &strings,
                            &mut stats,
                            &mut batch,
                            repo,
                        )
                        .unwrap();

                        if let Some(offset) = offset {
                            dir_entry.set_offset(offset);
                        };

                        Ok(())
                    })
                    .unwrap();

                repo.synchronize_data(&batch.to_vec(), &output).unwrap();

                let mut batch =
                    ChunkedVec::<(HashId, InlinedBoxedSlice), BATCH_CHUNK_CAPACITY>::default();
                output.clear();

                let offset = serialize_fun(
                    &Object::Directory(dir_id),
                    computed_hash_id,
                    &mut output,
                    &storage,
                    &strings,
                    &mut stats,
                    &mut batch,
                    repo,
                )
                .unwrap();
                repo.synchronize_data(&batch.to_vec(), &output).unwrap();
                output.clear();

                let object_ref = ObjectReference::new(Some(computed_hash_id), offset);
                let object = repo
                    .get_object(object_ref, &mut storage, &mut strings)
                    .unwrap();

                match object {
                    Object::Directory(new_dir) => {
                        if let Some(inode_id) = new_dir.get_inode_id() {
                            // Force to read the full inodes in repo
                            storage
                                .dir_to_vec_unsorted(new_dir, &mut strings, repo)
                                .unwrap();

                            // Remove existing hash ids from all the inodes children,
                            // to force recomputation of the hash.
                            storage.inodes_drop_hash_ids(inode_id);
                        }

                        let new_computed_hash_id =
                            hash_directory(new_dir, repo, &storage, &strings).unwrap();
                        let new_computed_hash_ref =
                            ObjectReference::new(Some(new_computed_hash_id), None);
                        let new_computed_hash = repo.get_hash(new_computed_hash_ref).unwrap();
                        let new_computed_hash =
                            ContextHash::try_from_bytes(new_computed_hash.as_ref()).unwrap();

                        // Hash must be the same than before the serialization & deserialization
                        assert_eq!(new_computed_hash, computed_hash);
                    }
                    _ => panic!(),
                }
            }

            assert_eq!(
                expected_hash.to_base58_check(),
                computed_hash.to_base58_check(),
                "Expected hash {} but got {} (bindings: {})",
                expected_hash.to_base58_check(),
                computed_hash.to_base58_check(),
                bindings_count
            );
        }
    }

    fn open_hashes_json_gz(file_name: &str) -> GzDecoder<File> {
        let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests")
            .join("resources")
            .join(file_name);
        let file = File::open(path)
            .unwrap_or_else(|_| panic!("Couldn't open file: tests/resources/{}", file_name));
        GzDecoder::new(file)
    }
}
