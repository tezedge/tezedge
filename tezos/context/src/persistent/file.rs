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

#[derive(Debug, Serialize, Deserialize)]
pub enum FileType {
    ShapeDirectories,
    ShapeDirectoriesIndex,
    CommitIndex,
    Data,
    Strings,
    BigStrings,
    Hashes,
}

impl Into<u64> for FileType {
    fn into(self) -> u64 {
        match self {
            FileType::ShapeDirectories => 0,
            FileType::ShapeDirectoriesIndex => 1,
            FileType::CommitIndex => 2,
            FileType::Data => 3,
            FileType::Strings => 4,
            FileType::BigStrings => 5,
            FileType::Hashes => 6,
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
        }
    }
}

pub struct File {
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

impl File {
    pub fn try_new(base_path: &str, file_type: FileType) -> Result<Self, OpenFileError> {
        std::fs::create_dir_all(&base_path)?;

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .append(true)
            .create(true)
            .custom_flags(get_custom_flags())
            .open(PathBuf::from(base_path).join(file_type.get_path()))?;

        // We use seek, in cases metadatas were not synchronized
        let offset = file.seek(SeekFrom::End(0))?;
        let mut file = Self { file, offset };

        if offset == 0 {
            file.write_header(file_type)?;
        } else {
            file.check_header(file_type)?;
        }

        Ok(file)
    }

    fn write_header(&mut self, file_type: FileType) -> Result<(), io::Error> {
        let mut bytes = Vec::with_capacity(16);

        let version_bytes = CURRENT_VERSION.to_le_bytes();
        bytes.extend_from_slice(&version_bytes[..]);

        let file_type: u64 = file_type.into();
        let file_type_bytes = file_type.to_le_bytes();
        bytes.extend_from_slice(&file_type_bytes[..]);

        debug_assert_eq!(bytes.len(), HEADER_LENGTH);

        self.append(bytes)?;

        Ok(())
    }

    fn check_header(&self, current_file_type: FileType) -> Result<(), OpenFileError> {
        let current_file_type: u64 = current_file_type.into();

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
