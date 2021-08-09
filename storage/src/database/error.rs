// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::persistent::SchemaError;
use crypto::hash::FromBytesError;
use std::io;
use thiserror::Error;

/// Possible errors for schema
#[derive(Debug, Error)]
pub enum Error {
    #[error("Guard Poison {error}")]
    GuardPoison { error: String },
    #[error("Value Missing {error}")]
    EdgeKVError { error: String },
    #[error("Missing sub tree {error}")]
    MissingSubTree { error: String },
    #[error("SledDB error: {error}")]
    SledDBError { error: sled::Error },
    #[error("I/O error {error}")]
    IOError { error: io::Error },
    #[error("Schema error: {error}")]
    SchemaError { error: SchemaError },
    #[error("Column name required")]
    ColumnRequired,
    #[error("Database failed to open")]
    FailedToOpenDatabase,
    #[error("Hash encode error : {error}")]
    HashEncodeError { error: FromBytesError },
    #[error("Database incompatibility {name}")]
    DatabaseIncompatibility { name: String },
    #[error("RocksDB error: {error}")]
    RocksDBError { error: rocksdb::Error },
    #[error("Column family {name} is missing")]
    MissingColumnFamily { name: &'static str },
}

impl From<SchemaError> for Error {
    fn from(error: SchemaError) -> Self {
        Error::SchemaError { error }
    }
}
impl From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Error::SledDBError { error }
    }
}
impl From<rocksdb::Error> for Error {
    fn from(error: rocksdb::Error) -> Self {
        Error::RocksDBError { error }
    }
}
impl From<FromBytesError> for Error {
    fn from(error: FromBytesError) -> Self {
        Error::HashEncodeError { error }
    }
}
