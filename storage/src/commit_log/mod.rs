// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Fail;
use serde::{Deserialize, Serialize};

use crate::persistent::codec::{Decoder, Encoder, SchemaError};
use crate::persistent::schema::{CommitLogDescriptor, CommitLogSchema};
use crate::persistent::BincodeEncoded;
use std::{fmt, io};

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, SeekFrom, Seek, Write, BufReader, Read};

pub type CommitLogRef = Arc<RwLock<CommitLog>>;

const DATA_FILE_NAME: &str = "table.data";


pub struct CommitLog {
    data_file: File,
    data_file_path : PathBuf
}

impl CommitLog {
    pub fn new<P: AsRef<Path>>(dir: P) -> Result<Self, CommitLogError> {
        if !dir.as_ref().exists() {
            std::fs::create_dir_all(dir.as_ref())?;
        }
        let mut data_file_path = PathBuf::new();
        data_file_path.push(dir.as_ref());
        data_file_path.push(DATA_FILE_NAME);
        let data_file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(data_file_path.clone())?;
        Ok(Self {
            data_file,
            data_file_path
        })
    }
    pub fn append_msg<B: AsRef<[u8]>>(&mut self, payload: B) -> Result<u64, CommitLogError> {
        let mut writer = BufWriter::new(&mut self.data_file);
        let offset = writer.seek(SeekFrom::End(0))?;
        writer.write_all(payload.as_ref())?;
        writer.flush()?;
        Ok(offset)
    }

    pub fn read(&self, offset: u64, buf_size: usize) -> Result<Vec<u8>, CommitLogError> {
        let mut buf = vec![0_u8; buf_size];
        let mut reader = BufReader::new(File::open(self.data_file_path.as_path())?);
        reader.seek(SeekFrom::Start(offset))?;
        reader.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn flush(&mut self) -> Result<(), CommitLogError> {
        self.data_file.flush()?;
        Ok(())
    }
}

/// Possible errors for commit log
#[derive(Debug, Fail)]
pub enum CommitLogError {
    #[fail(display = "Schema error: {}", error)]
    SchemaError { error: SchemaError },
    #[fail(display = "Commit log I/O error {}", error)]
    IOError { error: io::Error },
    #[fail(display = "Commit log {} is missing", name)]
    MissingCommitLog { name: &'static str },
    #[fail(display = "Failed to read record at {}", location)]
    ReadError {
        location: Location,
    },
    #[fail(display = "Failed to read record data corrupted")]
    CorruptData,
}

impl From<SchemaError> for CommitLogError {
    fn from(error: SchemaError) -> Self {
        CommitLogError::SchemaError { error }
    }
}

impl From<io::Error> for CommitLogError {
    fn from(error: io::Error) -> Self {
        CommitLogError::IOError { error }
    }
}

impl slog::Value for CommitLogError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

type ByteLimit = usize;
type ItemCount = u32;

/// Precisely identifies location of a record in a commit log.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Location(pub u64, pub ByteLimit);

impl Location {
    #[inline]
    pub fn is_consecutive(&self, prev: &Location) -> bool {
        (prev.0 < self.0) && (self.0 - prev.0 == 1)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_fmt(format_args!("Location({},{})", self.0, self.1))
    }
}

impl BincodeEncoded for Location {}

/// Range of values to get from a commit log
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Range(pub u64, pub ByteLimit, pub ItemCount);

/// Implement this trait for a commit log engine.
pub trait CommitLogWithSchema<S: CommitLogSchema> {
    /// Append new record to a commit log.
    fn append(&self, value: &S::Value) -> Result<Location, CommitLogError>;

    /// Retrieve a stored record.
    fn get(&self, location: &Location) -> Result<S::Value, CommitLogError>;

    /// Retrieve stored records stored in a single range.
    fn get_range(&self, range: &Range) -> Result<Vec<S::Value>, CommitLogError>;
}

impl<S: CommitLogSchema> CommitLogWithSchema<S> for CommitLogs {
    fn append(&self, value: &S::Value) -> Result<Location, CommitLogError> {
        let cl = self
            .cl_handle(S::name())
            .ok_or(CommitLogError::MissingCommitLog { name: S::name() })?;
        let mut cl = cl.write().expect("Write lock failed");
        let bytes = value.encode()?;
        let offset = cl
            .append_msg(&bytes)?;

        Ok(Location(offset, bytes.len()))
    }

    fn get(&self, location: &Location) -> Result<S::Value, CommitLogError> {
        let cl = self
            .cl_handle(S::name())
            .ok_or(CommitLogError::MissingCommitLog { name: S::name() })?;
        let cl = cl.read().expect("Read lock failed");
        let bytes =
            cl.read(location.0, location.1)?;
        let value = S::Value::decode(&bytes)?;
        Ok(value)
    }

    fn get_range(&self, _range: &Range) -> Result<Vec<S::Value>, CommitLogError> {
       unimplemented!()
    }
}

pub fn fold_consecutive_locations(locations: &[Location]) -> Vec<Range> {
    if locations.is_empty() {
        Vec::with_capacity(0)
    } else {
        let mut ranges = vec![];

        let mut prev = locations[0];
        let mut range = Range(prev.0, prev.1, 1);
        for curr in &locations[1..] {
            if curr.is_consecutive(&prev) {
                range.1 += curr.1;
                range.2 += 1;
            } else {
                ranges.push(range);
                range = Range(curr.0, curr.1, 1);
            }
            prev = *curr;
        }
        ranges.push(range);

        ranges
    }
}

/// Provides access to all registered commit logs via a log family reference.
pub struct CommitLogs {
    base_path: PathBuf,
    commit_log_map: RwLock<HashMap<String, CommitLogRef>>,
}

impl CommitLogs {
    pub(crate) fn new<P, I>(path: P, cfs: I) -> Result<Self, CommitLogError>
    where
        P: AsRef<Path>,
        I: IntoIterator<Item = CommitLogDescriptor>,
    {
        let myself = Self {
            base_path: path.as_ref().into(),
            commit_log_map: RwLock::new(HashMap::new()),
        };

        for descriptor in cfs.into_iter() {
            Self::register(&myself, descriptor.name())?;
        }

        Ok(myself)
    }

    /// Register a new commit log.
    fn register(&self, name: &str) -> Result<(), CommitLogError> {
        let path = self.base_path.join(name);
        if !Path::new(&path).exists() {
            std::fs::create_dir_all(&path)?;
        }
        let log = CommitLog::new(path)?;

        let mut commit_log_map = self.commit_log_map.write().unwrap();
        commit_log_map.insert(name.into(), Arc::new(RwLock::new(log)));

        Ok(())
    }

    /// Retrieve handle to a registered commit log.
    #[inline]
    fn cl_handle(&self, name: &str) -> Option<CommitLogRef> {
        let commit_log_map = self.commit_log_map.read().unwrap();
        commit_log_map.get(name).cloned()
    }

    /// Flush all registered commit logs.
    pub fn flush(&self) -> Result<(), CommitLogError> {
        let commit_log_map = self.commit_log_map.read().unwrap();
        for commit_log in commit_log_map.values() {
            let mut commit_log = commit_log.write().unwrap();
            commit_log.flush()?;
        }

        Ok(())
    }
}

impl Drop for CommitLogs {
    fn drop(&mut self) {
        let _ = self.flush().expect("Failed to flush commit logs");
    }
}

#[cfg(test)]
impl Location {
    pub fn new(offset: u64) -> Self {
        Self(offset, 0)
    }

    pub(crate) fn offset(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::commit_log::fold_consecutive_locations;
    use commitlog::message::MessageSet;
    use commitlog::{CommitLog as OldCommitLog, LogOptions, ReadLimit};
    use rand::Rng;
    use std::time::Instant;

    use super::*;

    #[test]
    fn test_fold_consecutive_locations_empty() {
        let locations = vec![];
        let ranges = fold_consecutive_locations(&locations);
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_fold_consecutive_locations_single() {
        let locations = vec![Location(1, 10)];
        let ranges = fold_consecutive_locations(&locations);
        assert_eq!(vec![Range(1, 10, 1)], ranges);
    }

    #[test]
    fn test_fold_consecutive_locations_multi() {
        let locations = vec![
            Location(1, 10),
            Location(2, 10),
            Location(5, 10),
            Location(7, 10),
            Location(8, 10),
            Location(9, 10),
            Location(6, 10),
        ];
        let ranges = fold_consecutive_locations(&locations);
        assert_eq!(
            vec![
                Range(1, 20, 2),
                Range(5, 10, 1),
                Range(7, 30, 3),
                Range(6, 10, 1)
            ],
            ranges
        );
    }

    fn generate_random_data(
        data_size: usize,
        min_message_size: usize,
        max_message_size: usize,
    ) -> Vec<Vec<u8>> {
        let mut rand_messages = vec![];
        let mut rng = rand::thread_rng();
        for _ in 0..data_size {
            let random_data_size = rng.gen_range(min_message_size, max_message_size);
            let random_bytes: Vec<u8> = (0..random_data_size).map(|_| 2_u8).collect();
            rand_messages.push(random_bytes);
        }
        rand_messages
    }

    #[test]
    fn compare_with_old_log() {
        let data_size = 10_000;
        let new_commit_log_dir = "./testdir/bench/new_log";
        let old_commit_log_dir = "./testdir/bench/old_log";
        let messages = generate_random_data(data_size, 10_000, 10_900);
        let mut options = LogOptions::new(old_commit_log_dir);
        options.message_max_bytes(15_000_000);
        let mut old_commit_log = OldCommitLog::new(options).unwrap();
        let mut new_commit_log = CommitLog::new(new_commit_log_dir).unwrap();
        println!("-------------------------------------------------------");
        println!("Write Benchmark");
        println!("-------------------------------------------------------");
        let mut timer = Instant::now();
        for msg in &messages {
            old_commit_log.append_msg(msg).unwrap();
        }
        println!(
            "Old CommitLog Store [{}] Took {}ms",
            messages.len(),
            timer.elapsed().as_millis()
        );

        timer = Instant::now();
        for msg in &messages {
            new_commit_log.append_msg(msg).unwrap();
        }
        println!(
            "New CommitLog Store [{}] Took {}ms",
            messages.len(),
            timer.elapsed().as_millis()
        );

        let old_commit_folder_size =
            fs_extra::dir::get_size(old_commit_log_dir).unwrap_or_default();
        let new_commit_folder_size =
            fs_extra::dir::get_size(new_commit_log_dir).unwrap_or_default();
        println!("-------------------------------------------------------");
        println!("Size Benchmark");
        println!("-------------------------------------------------------");
        println!("OldCommitLog {}", old_commit_folder_size);
        println!("NewCommitLog {}", new_commit_folder_size);

        println!("-------------------------------------------------------");
        println!("Read Benchmark");
        println!("-------------------------------------------------------");
        //Read Test on new commit log
        let mut timer = Instant::now();
        let set = new_commit_log.read(0, data_size).unwrap();
        for (i, _) in set.enumerate() {
            if i % 200 == 0 {
                print!(".")
            }
        }
        println!();
        println!(
            "New CommitLog Read [{}] items in {}ms",
            data_size,
            timer.elapsed().as_millis()
        );

        //Read Test on old commit log
        timer = Instant::now();
        let set = old_commit_log
            .read(0, ReadLimit::max_bytes(200_000_000))
            .unwrap();
        for (i, _) in set.iter().enumerate() {
            if i % 200 == 0 {
                print!(".")
            }
        }
        println!();
        println!(
            "Old CommitLog Read [{}] items in {}ms",
            data_size,
            timer.elapsed().as_millis()
        );

        std::fs::remove_dir_all(new_commit_log_dir).unwrap();
        std::fs::remove_dir_all(old_commit_log_dir).unwrap();
    }
}
