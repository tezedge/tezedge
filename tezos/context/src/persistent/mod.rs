// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{borrow::Cow, fs::OpenOptions, io::{self, Write}, path::{Path, PathBuf}, sync::{Arc, Mutex, PoisonError}};

use crypto::hash::ContextHash;
use thiserror::Error;

use tezos_timing::RepositoryMemoryUsage;

use crate::{
    kv_store::{readonly_ipc::ContextServiceError, HashId, HashIdError, VacantObjectHash},
    working_tree::{
        serializer::DeserializationError,
        shape::{DirectoryShapeError, DirectoryShapeId, ShapeStrings},
        storage::DirEntryId,
        string_interner::{StringId, StringInterner},
    },
    ObjectHash,
};

pub trait Flushable {
    fn flush(&self) -> Result<(), anyhow::Error>;
}

pub trait Persistable {
    fn is_persistent(&self) -> bool;
}

pub trait KeyValueStoreBackend {
    /// Write batch into DB atomically
    ///
    /// # Arguments
    /// * `batch` - WriteBatch containing all batched writes to be written to DB
    fn write_batch(&mut self, batch: Vec<(HashId, Arc<[u8]>)>) -> Result<(), DBError>;
    /// Check if database contains given hash id
    ///
    /// # Arguments
    /// * `hash_id` - HashId, to be checked for existence
    fn contains(&self, hash_id: HashId) -> Result<bool, DBError>;
    /// Mark the HashId as a ContextHash
    ///
    /// # Arguments
    /// * `hash_id` - HashId to mark
    fn put_context_hash(&mut self, hash_id: HashId, offset: u64) -> Result<(), DBError>;
    /// Get the HashId corresponding to the ContextHash
    ///
    /// # Arguments
    /// * `context_hash` - ContextHash to find the HashId
    fn get_context_hash(&self, context_hash: &ContextHash) -> Result<Option<(HashId, u64)>, DBError>;
    /// Read hash associated with given HashId, if exists.
    ///
    /// # Arguments
    /// * `hash_id` - HashId of the ObjectHash
    fn get_hash(&self, hash_id: HashId) -> Result<Option<Cow<ObjectHash>>, DBError>;
    /// Read value associated with given HashId, if exists.
    ///
    /// # Arguments
    /// * `hash_id` - HashId of the value
    fn get_value(&self, hash_id: HashId) -> Result<Option<Cow<[u8]>>, DBError>;
    /// Find an object to insert a new ObjectHash
    /// Return the object
    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError>;
    /// Manually clear the objects, this should be a no-operation if the implementation
    /// has its own garbage collection
    fn clear_objects(&mut self) -> Result<(), DBError>;
    /// Memory usage
    fn memory_usage(&self) -> RepositoryMemoryUsage;
    /// Returns the strings of the directory shape
    fn get_shape(&self, shape_id: DirectoryShapeId) -> Result<ShapeStrings, DBError>;
    /// Returns the `ShapeId` of this `dir`
    ///
    /// Create a new shape when it doesn't exist.
    /// This returns `None` when a shape cannot be made (currently if one of the
    /// string is > 30 bytes).
    fn make_shape(
        &mut self,
        dir: &[(StringId, DirEntryId)],
    ) -> Result<Option<DirectoryShapeId>, DBError>;
    /// Returns the string associated to this `string_id`.
    ///
    /// The string interner must have been updated with the `update_strings` method.
    fn get_str(&self, string_id: StringId) -> Option<&str>;
    /// Update the `StringInterner`.
    fn synchronize_strings(&mut self, string_interner: &StringInterner) -> Result<(), DBError>;

    fn get_current_offset(&self) -> Result<u64, DBError>;
    fn append_serialized_data(&mut self, data: &[u8]) -> Result<(), DBError>;
    fn synchronize_full(&mut self) -> Result<(), DBError>;
    fn get_value_from_offset(&self, buffer: &mut Vec<u8>, offset: u64) -> Result<(), DBError>;
}

/// Possible errors for schema
#[derive(Debug, Error)]
pub enum DBError {
    #[error("Column family {name} is missing")]
    MissingColumnFamily { name: &'static str },
    #[error("Database incompatibility {name}")]
    DatabaseIncompatibility { name: String },
    #[error("Value already exists {key}")]
    ValueExists { key: String },
    #[error("Found wrong structure. Was looking for {sought}, but found {found}")]
    FoundUnexpectedStructure { sought: String, found: String },
    #[error("Guard Poison {error} ")]
    GuardPoison { error: String },
    #[error("Mutex/lock lock error! Reason: {reason}")]
    LockError { reason: String },
    #[error("I/O error {error}")]
    IOError {
        #[from]
        error: io::Error,
    },
    #[error("MemoryStatisticsOverflow")]
    MemoryStatisticsOverflow,
    #[error("IPC Context access error: {reason:?}")]
    IpcAccessError { reason: ContextServiceError },
    #[error("Missing object: {hash_id:?}")]
    MissingObject { hash_id: HashId },
    #[error("Conversion from/to HashId failed")]
    HashIdFailed,
    #[error("Deserialization error: {error:?}")]
    DeserializationError {
        #[from]
        error: DeserializationError,
    },
    #[error("Shape error: {error:?}")]
    ShapeError {
        #[from]
        error: DirectoryShapeError,
    },
}

impl From<HashIdError> for DBError {
    fn from(_: HashIdError) -> Self {
        DBError::HashIdFailed
    }
}

impl<T> From<PoisonError<T>> for DBError {
    fn from(pe: PoisonError<T>) -> Self {
        DBError::LockError {
            reason: format!("{}", pe),
        }
    }
}

#[derive(Debug)]
pub enum FileType {
    ShapeDirectories,
    CommitIndex,
    Data,
    Strings,
    Hashes,
}

const PERSISTENT_BASE_PATH: &str = "db_persistent";
const PERSISTENT_BASE_PATH_TEST: &str = "db_persistent/{}";

impl FileType {
    fn get_path(&self) -> &Path {
        match self {
            FileType::ShapeDirectories => Path::new("shape_directories.db"),
            FileType::CommitIndex => Path::new("commit_index.db"),
            FileType::Data => Path::new("data.db"),
            FileType::Strings => Path::new("strings.db"),
            FileType::Hashes => Path::new("hashes.db"),
        }
    }
}

pub struct File {
    file: std::fs::File,
    offset: u64,
    file_type: FileType,
}

/// Absolute offset in the file
#[derive(Debug)]
pub struct FileOffset(pub u64);

// static BASE_PATH_EXCLUSIVITY: Arc

// #[cfg(test)]
lazy_static::lazy_static! {
    static ref BASE_PATH_EXCLU: Arc<Mutex<()>> = {
        Arc::new(Mutex::new(()))
    };
}

// #[cfg(test)]
fn create_random_path() -> String {
    use rand::Rng;

    let mut rng = rand::thread_rng();

    // Avoid data races with `Path::exists` below
    let _guard = BASE_PATH_EXCLU.lock().unwrap();

    let mut path = format!("{}/{}", PERSISTENT_BASE_PATH, rng.gen::<u32>());

    while Path::new(&path).exists() {
        path = format!("{}/{}", PERSISTENT_BASE_PATH, rng.gen::<u32>());
    }

    path
}

pub fn get_persistent_base_path() -> String {
    // #[cfg(not(test))]
    // return PERSISTENT_BASE_PATH.to_string();

    // #[cfg(test)]
    return create_random_path();
}

impl File {
    pub fn new(base_path: &str, file_type: FileType) -> Self {
        println!("FILE={:?}", PathBuf::from(base_path).join(file_type.get_path()));
        // println!("BASE={:?}", base_path);

        std::fs::create_dir_all(&base_path).unwrap();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(true)
            .create(true)
            .open(PathBuf::from(base_path).join(file_type.get_path()))
            .unwrap();

        Self { file, offset: 0, file_type }
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn append(&mut self, bytes: impl AsRef<[u8]>) -> FileOffset {
        let bytes = bytes.as_ref();

        let offset = self.offset;
        self.offset += bytes.len() as u64;

        self.file.write_all(bytes).unwrap();

        FileOffset(offset)
    }

    pub fn read_at(&self, buffer: &mut Vec<u8>, offset: FileOffset) {
        use std::os::unix::prelude::FileExt;

        self.file.read_at(buffer, offset.0).unwrap();
    }

    pub fn read_exact_at(&self, buffer: &mut [u8], offset: FileOffset) {
        use std::os::unix::prelude::FileExt;

        // println!("{:?} READING {:?} AT OFFSET {:?}", self.file_type, buffer.len(), offset);

        self.file.read_exact_at(buffer, offset.0).unwrap();
    }
}

// struct FileSystem {
//     data_file: File,
//     shape_file: File,
//     commit_index_file: File,
//     strings_file: File,
// }

// impl FileSystem {
//     fn new() -> FileSystem {
//         let data_file = File::new(FileType::Data);
//         let shape_file = File::new(FileType::ShapeDirectories);
//         let commit_index_file = File::new(FileType::CommitIndex);
//         let strings_file = File::new(FileType::Strings);

//         Self {
//             data_file,
//             shape_file,
//             commit_index_file,
//             strings_file,
//         }
//     }
// }

// impl FileType {
//     fn get_path(&self) -> &Path {
//         match self {
//             FileType::ShapeDirectories => Path::new("shape_directories.db"),
//             FileType::CommitIndex => Path::new("commit_index.db"),
//             FileType::Data => Path::new("data.db"),
//             FileType::Strings => Path::new("strings.db"),
//         }
//     }
// }
