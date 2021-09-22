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
    inode: &Inode,
    store: &mut ContextKeyValueStore,
    storage: &Storage,
) -> Result<HashId, HashingError> {
    let mut hasher = VarBlake2b::new(OBJECT_HASH_LEN)?;

    match inode {
        Inode::Directory(dir_id) => {
            // Inode value:
            //
            // |   1   |   1  |     n_1      |  ...  |      n_k      |
            // +-------+------+--------------+-------+---------------+
            // | \000  |  \n  | prehash(e_1) |  ...  | prehash(e_k)  |
            //
            // where n_i = len(prehash(e_i))

            let dir = storage.get_small_dir(*dir_id)?;

            hasher.update(&[0u8]); // type tag
            hasher.update(&[dir.len() as u8]);

            // Inode value object:
            //
            // |   (LEB128)  |  len(name)   |   1    |   32   |
            // +-------------+--------------+--------+--------+
            // | \len(name)  |     name     |  kind  |  hash  |

            for (name, dir_entry_id) in dir {
                let name = storage.get_str(*name)?;

                leb128::write::unsigned(&mut hasher, name.len() as u64)?;
                hasher.update(name.as_bytes());

                // \000 for nodes, and \001 for contents.
                let dir_entry = storage.get_dir_entry(*dir_entry_id)?;
                match dir_entry.dir_entry_kind() {
                    DirEntryKind::Blob => hasher.update(&[1u8]),
                    DirEntryKind::Directory => hasher.update(&[0u8]),
                };

                let blob_inlined = dir_entry.get_object().and_then(|object| match object {
                    Object::Blob(blob_id) if blob_id.is_inline() => storage.get_blob(blob_id).ok(),
                    _ => None,
                });

                if let Some(blob) = blob_inlined {
                    hasher.update(&hash_inlined_blob(blob)?);
                } else {
                    hasher.update(dir_entry.object_hash(store, storage)?.as_ref());
                }
            }
        }
        Inode::Pointers {
            depth,
            nchildren,
            npointers,
            pointers,
        } => {
            // Inode directory:
            //
            // |   1    | (LEB128) |   (LEB128)    |    1   |  33  | ... |  33  |
            // +--------+----------+---------------+--------+------+-----+------+
            // |  \001  |  depth   | len(children) |   \k   | s_1  | ... | s_k  |

            hasher.update(&[1u8]); // type tag
            leb128::write::unsigned(&mut hasher, *depth as u64)?;
            leb128::write::unsigned(&mut hasher, *nchildren as u64)?;
            hasher.update(&[*npointers as u8]);

            // Inode pointer:

            //
            // |    1    |   32   |
            // +---------+--------+
            // |  index  |  hash  |

            for (index, pointer) in pointers.iter().enumerate() {
                // When the pointer is `None`, it means that there is no DirEntry
                // under that index.

                // Skip pointers without entries.
                if let Some(pointer) = pointer.as_ref() {
                    let index: u8 = index as u8;

                    hasher.update(&[index]);

                    let hash_id = match pointer.hash_id() {
                        Some(hash_id) => hash_id,
                        None => {
                            let inode_id = pointer.inode_id();
                            let inode = storage.get_inode(inode_id)?;
                            let hash_id = hash_long_inode(inode, store, storage)?;
                            pointer.set_hash_id(Some(hash_id));
                            hash_id
                        }
                    };

                    let hash = store
                        .get_hash(hash_id)?
                        .ok_or(HashingError::HashIdNotFound { hash_id })?;

                    hasher.update(hash.as_ref());
                };
            }
        }
    }

    let hash_id = store
        .get_vacant_object_hash()?
        .write_with(|object| hasher.finalize_variable(|r| object.copy_from_slice(r)));

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

    for (k, v) in dir {
        let v = storage.get_dir_entry(*v)?;
        hasher.update(encode_irmin_dir_entry_kind(&v.dir_entry_kind()));
        // Key length is written in LEB128 encoding

        let k = storage.get_str(*k)?;
        leb128::write::unsigned(&mut hasher, k.len() as u64)?;
        hasher.update(k.as_bytes());
        hasher.update(&(OBJECT_HASH_LEN as u64).to_be_bytes());

        let blob_inlined = v.get_object().and_then(|object| match object {
            Object::Blob(blob_id) if blob_id.is_inline() => storage.get_blob(blob_id).ok(),
            _ => None,
        });

        if let Some(blob) = blob_inlined {
            hasher.update(&hash_inlined_blob(blob)?);
        } else {
            hasher.update(v.object_hash(store, storage)?.as_ref());
        }
    }

    let hash_id = store
        .get_vacant_object_hash()?
        .write_with(|object| hasher.finalize_variable(|r| object.copy_from_slice(r)));

    Ok(hash_id)
}

// Calculates hash of directory
// uses BLAKE2 binary 256 length hash function
pub(crate) fn hash_directory(
    dir_id: DirectoryId,
    store: &mut ContextKeyValueStore,
    storage: &Storage,
) -> Result<HashId, HashingError> {
    if let Some(inode_id) = dir_id.get_inode_id() {
        let inode = storage.get_inode(inode_id)?;
        hash_long_inode(&inode, store, storage)
    } else {
        hash_short_inode(dir_id, store, storage)
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
        .write_with(|object| hasher.finalize_variable(|r| object.copy_from_slice(r)));

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

    let root_hash = store
        .get_hash(commit.root_hash)?
        .ok_or(HashingError::ValueExpected("root_hash"))?;
    hasher.update(root_hash.as_ref());

    if let Some(parent) = commit.parent_commit_hash {
        let parent_commit_hash = store
            .get_hash(parent)?
            .ok_or(HashingError::ValueExpected("parent_commit_hash"))?;
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
        .write_with(|object| hasher.finalize_variable(|r| object.copy_from_slice(r)));

    Ok(hash_id)
}

pub(crate) fn hash_object(
    object: &Object,
    store: &mut ContextKeyValueStore,
    storage: &Storage,
) -> Result<Option<HashId>, HashingError> {
    match object {
        Object::Commit(commit) => hash_commit(commit, store).map(Some),
        Object::Directory(dir_id) => hash_directory(*dir_id, store, storage).map(Some),
        Object::Blob(blob_id) => hash_blob(*blob_id, store, storage),
    }
}

#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    use std::convert::TryInto;
    use std::{collections::HashSet, env, fs::File, io::Read, path::Path};

    use flate2::read::GzDecoder;

    use crypto::hash::{ContextHash, HashTrait};
    use tezos_timing::SerializeStats;

    use crate::persistent::KeyValueStoreBackend;
    use crate::kv_store::persistent::Persistent;
    use crate::{
        kv_store::in_memory::InMemory,
        working_tree::{
            serializer::{deserialize_object, serialize_object},
            DirEntry, DirEntryKind,
        },
    };

    use super::*;

    #[test]
    fn test_hash_of_commit() {
        let mut repo = Persistent::try_new().expect("failed to create context");
        // let mut repo = InMemory::try_new().expect("failed to create context");

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

        let hash_id = repo.put_object_hash(
            hex::decode("0d78b30e959c2a079e8ccb4ca19d428c95d29b2f02a35c1c58ef9c8972bc26aa")
                .unwrap()
                .try_into()
                .unwrap(),
        );

        let dummy_commit = Commit {
            parent_commit_hash: None,
            root_hash: hash_id,
            root_hash_offset: 0,
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

        bytes += &hash_length;
        bytes += &dir_hash;
        bytes += &parents_count;
        bytes += &commit_time;
        bytes += &commit_author_name_length;
        bytes += &commit_author_name;
        bytes += &commit_message_length;
        bytes += &commit_message;

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

        let hash_id = hash_commit(&dummy_commit, &mut repo).unwrap();

        assert_eq!(
            calculated_commit_hash.as_ref(),
            repo.get_hash(hash_id).unwrap().unwrap().as_ref()
        );
        assert_eq!(
            expected_commit_hash,
            hex::encode(calculated_commit_hash.as_ref())
        );
    }

    #[test]
    fn test_hash_of_small_dir() {
        // Calculates hash of directory
        // uses BLAKE2 binary 256 length hash function
        // hash is calculated as:
        // <number of child nodes (8 bytes)><CHILD NODE>
        // where:
        // - CHILD NODE - <NODE TYPE><length of string (1 byte)><string/path bytes><length of hash (8bytes)><hash bytes>
        // - NODE TYPE - blob dir_entry(0xff00000000000000) or internal dir_entry (0x0000000000000000)
        let mut repo = Persistent::try_new().expect("failed to create context");
        // let mut repo = InMemory::try_new().expect("failed to create context");
        let expected_dir_hash = "d49a53323107f2ae40b01eaa4e9bec4d02801daf60bab82dc2529e40d40fa917";
        let dummy_dir = DirectoryId::empty();

        let mut storage = Storage::new();

        let blob_id = storage.add_blob_by_ref(&[1]).unwrap();

        let dir_entry = DirEntry::new(DirEntryKind::Blob, Object::Blob(blob_id));

        let dummy_dir = storage.dir_insert(dummy_dir, "a", dir_entry).unwrap();

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

        bytes += &child_dir_entries;
        bytes += &blob_dir_entry;
        bytes += &string_length;
        bytes += &string_value;
        bytes += &hash_length;
        bytes += &hash;

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

        let hash_id = hash_directory(dummy_dir, &mut repo, &mut storage).unwrap();

        assert_eq!(
            calculated_dir_hash.as_ref(),
            repo.get_hash(hash_id).unwrap().unwrap().as_ref(),
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
    fn test_dir_entry_hashes() {
        test_type_hashes("nodes.json.gz");
    }

    #[test]
    fn test_inode_hashes() {
        test_type_hashes("inodes.json.gz");
    }

    fn test_type_hashes(json_gz_file_name: &str) {
        let mut json_file = open_hashes_json_gz(json_gz_file_name);
        let mut bytes = Vec::new();

        let mut repo = Persistent::try_new().expect("failed to create context");

        // let mut repo = InMemory::try_new().expect("failed to create context");
        let mut storage = Storage::new();
        // let mut output = Vec::new();
        // let mut older_objects = Vec::new();
        let mut stats = SerializeStats::default();

        // NOTE: reading from a stream is very slow with serde, thats why
        // the whole file is being read here before parsing.
        // See: https://github.com/serde-rs/json/issues/160#issuecomment-253446892
        json_file.read_to_end(&mut bytes).unwrap();

        let test_cases: Vec<DirEntryHashTest> = serde_json::from_slice(&bytes).unwrap();

        for test_case in test_cases {
            let bindings_count = test_case.bindings.len();
            let mut dir_id = DirectoryId::empty();
            // let mut batch = Vec::new();

            let mut names = HashSet::new();

            for binding in test_case.bindings {
                let dir_entry_kind = match binding.kind.as_str() {
                    "Tree" => DirEntryKind::Directory,
                    "Contents" => DirEntryKind::Blob,
                    other => panic!("Got unexpected binding kind: {}", other),
                };
                let object_hash = ContextHash::from_base58_check(&binding.hash).unwrap();

                let hash_id =
                    repo.put_object_hash(object_hash.as_ref().as_slice().try_into().unwrap());

                let dir_entry = DirEntry::new_commited(dir_entry_kind, Some(hash_id), None);

                names.insert(binding.name.clone());

                dir_id = storage
                    .dir_insert(dir_id, binding.name.as_str(), dir_entry)
                    .unwrap();

                assert!(storage
                    .dir_find_dir_entry(dir_id, binding.name.as_str())
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
                            DirEntry::new_commited(DirEntryKind::Blob, Some(hash_id), None),
                        )
                        .unwrap();
                    let a = dir_id;

                    // Insert the same element (same key) twice, this must not increment
                    // `Inode::Pointers::nchildren`.
                    dir_id = storage
                        .dir_insert(
                            dir_id,
                            &key,
                            DirEntry::new_commited(DirEntryKind::Blob, Some(hash_id), None),
                        )
                        .unwrap();
                    let b = dir_id;

                    assert_eq!(storage.dir_len(a).unwrap(), storage.dir_len(b).unwrap());
                }

                // Remove the elements we just inserted
                for index in 0..10000 {
                    let key = format!("abc{}", index);
                    dir_id = storage.dir_remove(dir_id, &key).unwrap();
                    let a = dir_id;

                    // Remove the same key twice
                    dir_id = storage.dir_remove(dir_id, &key).unwrap();
                    let b = dir_id;

                    // The 2nd remove should not modify the existing inode or create a new one
                    assert_eq!(a, b);
                }
            }

            let expected_hash = ContextHash::from_base58_check(&test_case.hash).unwrap();
            let computed_hash_id = hash_directory(dir_id, &mut repo, &mut storage).unwrap();
            let computed_hash = repo.get_hash(computed_hash_id).unwrap().unwrap();
            let computed_hash = ContextHash::try_from_bytes(computed_hash.as_ref()).unwrap();

            // // The following block makes sure that a serialized & deserialized inode
            // // produce the same hash
            // {
            //     serialize_object(
            //         &Object::Directory(dir_id),
            //         computed_hash_id,
            //         &mut output,
            //         &storage,
            //         &mut stats,
            //         &mut batch,
            //         &mut older_objects,
            //         &mut repo,
            //         0,
            //     )
            //     .unwrap();
            //     repo.write_batch(batch).unwrap();

            //     let data = repo.get_value(computed_hash_id).unwrap().unwrap();

            //     storage.data = data.to_vec(); // TODO: Do not do this
            //     let object = deserialize_object(&mut storage, &repo).unwrap();

            //     match object {
            //         Object::Directory(new_dir) => {
            //             if let Some(inode_id) = new_dir.get_inode_id() {
            //                 // Remove existing hash ids from all the inodes children,
            //                 // to force recomputation of the hash.
            //                 storage.inodes_drop_hash_ids(inode_id);
            //             }

            //             let new_computed_hash_id =
            //                 hash_directory(new_dir, &mut repo, &mut storage).unwrap();
            //             let new_computed_hash =
            //                 repo.get_hash(new_computed_hash_id).unwrap().unwrap();
            //             let new_computed_hash =
            //                 ContextHash::try_from_bytes(new_computed_hash.as_ref()).unwrap();

            //             // Hash must be the same than before the serialization & deserialization
            //             assert_eq!(new_computed_hash, computed_hash);
            //         }
            //         _ => panic!(),
            //     }
            // }

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
