// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;
use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, DB, Options};

pub use codec::{BincodeEncoded, Codec, Decoder, Encoder, SchemaError};
pub use commit_log::{CommitLogError, CommitLogRef, CommitLogs, CommitLogWithSchema, Location};
pub use database::{DBError, KeyValueStoreWithSchema};
pub use schema::{CommitLogDescriptor, CommitLogSchema, KeyValueSchema};

use crate::persistent::sequence::Sequences;

pub mod sequence;
pub mod codec;
pub mod schema;
pub mod database;
pub mod commit_log;

/// Open RocksDB database at given path with specified Column Family configurations
///
/// # Arguments
/// * `path` - Path to open RocksDB
/// * `cfs` - Iterator of Column Family descriptors
pub fn open_kv<P, I>(path: P, cfs: I) -> Result<DB, DBError>
    where
        P: AsRef<Path>,
        I: IntoIterator<Item=ColumnFamilyDescriptor>,
{
    DB::open_cf_descriptors(&default_kv_options(), path, cfs)
        .map_err(DBError::from)
}

/// Create default database configuration options
fn default_kv_options() -> Options {
    let mut db_opts = Options::default();
    db_opts.create_missing_column_families(true);
    db_opts.create_if_missing(true);
    db_opts
}

/// Open commit log at a given path.
pub fn open_cl<P, I>(path: P, cfs: I) -> Result<CommitLogs, CommitLogError>
    where
        P: AsRef<Path>,
        I: IntoIterator<Item=CommitLogDescriptor>
{
    CommitLogs::new(path, cfs)
}


/// Groups all components required for correct permanent storage functioning
#[derive(Clone)]
pub struct PersistentStorage {
    /// key-value store
    kv: Arc<DB>,
    /// commit log store
    clog: Arc<CommitLogs>,
    /// autoincrement  id generators
    seq: Arc<Sequences>,
}

impl PersistentStorage {
    pub fn new(kv: Arc<DB>, clog: Arc<CommitLogs>) -> Self {
        Self {
            seq: Arc::new(Sequences::new(kv.clone(), 1000)),
            kv,
            clog,
        }
    }

    #[inline]
    pub fn kv(&self) -> Arc<DB> {
        self.kv.clone()
    }

    #[inline]
    pub fn clog(&self) -> Arc<CommitLogs> {
        self.clog.clone()
    }

    #[inline]
    pub fn seq(&self) -> Arc<Sequences> {
        self.seq.clone()
    }
}

