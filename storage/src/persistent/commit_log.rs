// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fmt, io};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use commitlog::{AppendError, CommitLog, LogOptions, Offset, ReadError, ReadLimit};
use commitlog::message::MessageSet;
use failure::Fail;
use serde::{Deserialize, Serialize};

use crate::persistent::BincodeEncoded;
use crate::persistent::codec::{Decoder, Encoder, SchemaError};
use crate::persistent::schema::{CommitLogDescriptor, CommitLogSchema};



pub type CommitLogRef = Arc<RwLock<CommitLog>>;

/// Possible errors for commit log
#[derive(Debug, Fail)]
pub enum CommitLogError {
    #[fail(display = "Schema error: {}", error)]
    SchemaError {
        error: SchemaError
    },
    #[fail(display = "Failed to append record: {}", error)]
    AppendError {
        error: AppendError
    },
    #[fail(display = "Failed to read record at {}: {}", location, error)]
    ReadError {
        error: ReadError,
        location: Location,
    },
    #[fail(display = "Commit log I/O error {}", error)]
    IOError {
        error: io::Error
    },
    #[fail(display = "Commit log {} is missing", name)]
    MissingCommitLog {
        name: &'static str
    },
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
    fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}


/// Precisely identifies location of a record in a commit log.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Location(Offset, usize);

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_fmt(format_args!("Location({},{})", self.0, self.1))
    }
}

impl BincodeEncoded for Location {}

/// Implement this trait for a commit log engine.
pub trait CommitLogWithSchema<S: CommitLogSchema> {
    /// Append new record to a commit log.
    fn append(&self, value: &S::Value) -> Result<Location, CommitLogError>;

    /// Retrieve a stored record.
    fn get(&self, location: &Location) -> Result<S::Value, CommitLogError>;
}


impl<S: CommitLogSchema> CommitLogWithSchema<S> for CommitLogs {
    fn append(&self, value: &S::Value) -> Result<Location, CommitLogError> {
        let cl = self.cl_handle(S::name())
            .ok_or(CommitLogError::MissingCommitLog { name: S::name() })?;
        let mut cl = cl.write().expect("Write lock failed");
        let bytes = value.encode()?;
        let offset = cl.append_msg(&bytes)
            .map_err(|error| CommitLogError::AppendError { error })?;

        Ok(Location(offset, bytes.len()))
    }

    fn get(&self, location: &Location) -> Result<S::Value, CommitLogError> {
        let cl = self.cl_handle(S::name())
            .ok_or(CommitLogError::MissingCommitLog { name: S::name() })?;
        let cl = cl.read().expect("Read lock failed");
        let msg_buf = cl.read(location.0, fit_read_limit(location.1))
            .map_err(|error| CommitLogError::ReadError { error, location: *location })?;
        let bytes = msg_buf.iter().next().ok_or(CommitLogError::ReadError { error: ReadError::CorruptLog, location: *location })?;
        let value = S::Value::decode(bytes.payload())?;

        Ok(value)
    }
}

#[inline]
fn fit_read_limit(limit: usize) -> ReadLimit {
    ReadLimit::max_bytes(limit + 32)
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

        let mut opts = LogOptions::new(&path);
        opts.message_max_bytes(10_000_000);
        let log = CommitLog::new(opts)?;

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


#[cfg(test)]
impl Location {
    pub fn new(offset: Offset) -> Self {
        Self(offset, 0)
    }

    pub(crate) fn offset(&self) -> Offset {
        self.0
    }
}
