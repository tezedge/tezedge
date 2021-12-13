// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    convert::TryInto,
    fs::OpenOptions,
    io::{self, Seek, SeekFrom, Write},
    os::unix::prelude::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::serialize::persistent::AbsoluteOffset;

const CURRENT_VERSION: u64 = 0;
const HEADER_LENGTH: usize = 16;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileType {
    ShapeDirectories,
    ShapeDirectoriesIndex,
    CommitIndex,
    Data,
    Strings,
    BigStrings,
    Hashes,
    Sizes,
}

type TaggedFile = u64;

pub const TAG_SHAPE: u64 = 0;
pub const TAG_SHAPE_INDEX: u64 = 1;
pub const TAG_COMMIT_INDEX: u64 = 2;
pub const TAG_DATA: u64 = 3;
pub const TAG_STRINGS: u64 = 4;
pub const TAG_BIG_STRINGS: u64 = 5;
pub const TAG_HASHES: u64 = 6;
pub const TAG_SIZES: u64 = 7;

impl From<FileType> for u64 {
    fn from(file_type: FileType) -> Self {
        match file_type {
            FileType::ShapeDirectories => TAG_SHAPE,
            FileType::ShapeDirectoriesIndex => TAG_SHAPE_INDEX,
            FileType::CommitIndex => TAG_COMMIT_INDEX,
            FileType::Data => TAG_DATA,
            FileType::Strings => TAG_STRINGS,
            FileType::BigStrings => TAG_BIG_STRINGS,
            FileType::Hashes => TAG_HASHES,
            FileType::Sizes => TAG_SIZES,
        }
    }
}

impl From<u64> for FileType {
    fn from(value: u64) -> Self {
        match value {
            TAG_SHAPE => FileType::ShapeDirectories,
            TAG_SHAPE_INDEX => FileType::ShapeDirectoriesIndex,
            TAG_COMMIT_INDEX => FileType::CommitIndex,
            TAG_DATA => FileType::Data,
            TAG_STRINGS => FileType::Strings,
            TAG_BIG_STRINGS => FileType::BigStrings,
            TAG_HASHES => FileType::Hashes,
            TAG_SIZES => FileType::Sizes,
            _ => unreachable!(), // error at compile time
        }
    }
}

#[derive(Debug, Error)]
pub enum OpenFileError {
    #[error("IO Error {0}")]
    IO(#[from] io::Error),
    #[error("HeaderError {0}")]
    Header(#[from] HeaderError),
}

#[derive(Debug, Error)]
pub enum HeaderError {
    #[error("Version mismatch file_version={file_version} version={version}")]
    VersionMismatch { file_version: u64, version: u64 },
    #[error("Invalid type")]
    InvalidType,
}

const PERSISTENT_BASE_PATH: &str = "db_persistent";

impl FileType {
    fn get_path(&self) -> &Path {
        match self {
            FileType::ShapeDirectories => Path::new("shape_directories.db"),
            FileType::ShapeDirectoriesIndex => Path::new("shape_directories_index.db"),
            FileType::CommitIndex => Path::new("commit_index.db"),
            FileType::Data => Path::new("data.db"),
            FileType::Strings => Path::new("strings.db"),
            FileType::Hashes => Path::new("hashes.db"),
            FileType::BigStrings => Path::new("big_strings.db"),
            FileType::Sizes => Path::new("sizes.db"),
        }
    }
}

// Note: Use `File<const T: FileType` once the feature `adt_const_params` is stabilized
pub struct File<const T: TaggedFile> {
    file: std::fs::File,
    offset: u64,
}

/// Absolute offset in the file
#[derive(Debug)]
pub struct FileOffset(pub u64);

lazy_static::lazy_static! {
    static ref BASE_PATH_EXCLU: Arc<Mutex<()>> = {
        Arc::new(Mutex::new(()))
    };
}

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

pub fn get_persistent_base_path(db_path: Option<&str>) -> String {
    match db_path {
        Some(db_path) if !db_path.is_empty() => db_path.to_string(),
        _ => create_random_path(),
    }
}

#[cfg(target_os = "linux")]
fn get_custom_flags() -> i32 {
    libc::O_NOATIME
}

#[cfg(not(target_os = "linux"))]
fn get_custom_flags() -> i32 {
    0
}

impl<const T: TaggedFile> File<T> {
    pub fn try_new(base_path: &str) -> Result<Self, OpenFileError> {
        std::fs::create_dir_all(&base_path)?;

        let file_type: FileType = T.into();
        let append_mode = !matches!(file_type, FileType::Sizes);

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .append(append_mode)
            .create(true)
            .custom_flags(get_custom_flags())
            .open(PathBuf::from(base_path).join(file_type.get_path()))?;

        // We use seek, in cases metadatas were not synchronized
        let offset = file.seek(SeekFrom::End(0))?;
        let mut file = Self { file, offset };

        if offset == 0 {
            file.write_header()?;
        } else {
            file.check_header()?;
        }

        Ok(file)
    }

    fn write_header(&mut self) -> Result<(), io::Error> {
        let mut bytes = Vec::with_capacity(16);

        let version_bytes = CURRENT_VERSION.to_le_bytes();
        bytes.extend_from_slice(&version_bytes[..]);

        let file_type: u64 = T.into();
        let file_type_bytes = file_type.to_le_bytes();
        bytes.extend_from_slice(&file_type_bytes[..]);

        debug_assert_eq!(bytes.len(), HEADER_LENGTH);

        self.append(bytes)?;

        Ok(())
    }

    fn check_header(&self) -> Result<(), OpenFileError> {
        let current_file_type: u64 = T.into();

        let mut bytes: [u8; 16] = Default::default();
        self.read_exact_at(&mut bytes, 0.into())?;

        let version = u64::from_le_bytes(bytes[0..8].try_into().unwrap()); // never fail
        let file_type = u64::from_le_bytes(bytes[8..16].try_into().unwrap()); // never fail

        if file_type != current_file_type {
            return Err(HeaderError::InvalidType.into());
        }

        if version != CURRENT_VERSION {
            return Err(HeaderError::VersionMismatch {
                file_version: version,
                version: CURRENT_VERSION,
            }
            .into());
        }

        Ok(())
    }

    pub fn truncate(&mut self, new_size: u64) -> Result<(), io::Error> {
        if new_size != self.offset {
            assert!(new_size < self.offset);

            self.file.set_len(new_size)?;
            self.offset = new_size;
        }

        Ok(())
    }

    pub fn start(&self) -> u64 {
        HEADER_LENGTH as u64
    }

    pub fn offset(&self) -> AbsoluteOffset {
        self.offset.into()
    }

    #[cfg(test)]
    pub fn sync(&mut self) -> Result<(), io::Error> {
        Ok(())
    }

    #[cfg(not(test))]
    pub fn sync(&mut self) -> Result<(), io::Error> {
        self.file.sync_data()
    }

    pub fn append(&mut self, bytes: impl AsRef<[u8]>) -> Result<(), io::Error> {
        let bytes = bytes.as_ref();

        self.offset += bytes.len() as u64;
        self.file.write_all(bytes)
    }

    pub fn write_all_at(
        &mut self,
        bytes: impl AsRef<[u8]>,
        offset: AbsoluteOffset,
    ) -> Result<(), io::Error> {
        use std::os::unix::prelude::FileExt;

        let bytes = bytes.as_ref();
        self.file.write_all_at(bytes, offset.as_u64())
    }

    pub fn read_exact_at(
        &self,
        buffer: &mut [u8],
        offset: AbsoluteOffset,
    ) -> Result<(), io::Error> {
        use std::os::unix::prelude::FileExt;

        self.file.read_exact_at(buffer, offset.as_u64())
    }

    pub fn read_at_most<'a>(
        &self,
        mut buffer: &'a mut [u8],
        offset: AbsoluteOffset,
    ) -> Result<&'a [u8], io::Error> {
        let buf_len = buffer.len();

        let eof = self.offset as usize;
        let end = offset.as_u64() as usize + buf_len;

        if eof < end {
            buffer = &mut buffer[..buf_len - (end - eof)];
        }

        self.read_exact_at(buffer, offset)?;

        Ok(buffer)
    }
}
