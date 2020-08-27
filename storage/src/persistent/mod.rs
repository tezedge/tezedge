// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use derive_builder::Builder;
use rocksdb::{BlockBasedOptions, ColumnFamilyDescriptor, DB, Options};

pub use codec::{BincodeEncoded, Codec, Decoder, Encoder, SchemaError};
pub use commit_log::{CommitLogError, CommitLogRef, CommitLogs, CommitLogWithSchema, Location};
pub use database::{DBError, KeyValueStoreWithSchema};
pub use schema::{CommitLogDescriptor, CommitLogSchema, KeyValueSchema};

use crate::persistent::sequence::Sequences;
use crate::skip_list::{Bucket, DatabaseBackedSkipList, TypedSkipList};
use crate::merkle_storage::MerkleStorage;

pub mod sequence;
pub mod codec;
pub mod schema;
pub mod database;
pub mod commit_log;

/// Rocksdb database system configuration
/// - [max_num_of_threads] - if not set, num of cpus is used
#[derive(Builder, Debug, Clone)]
pub struct DbConfiguration {
    #[builder(default = "None")]
    max_threads: Option<usize>,
}

impl Default for DbConfiguration {
    fn default() -> Self {
        DbConfigurationBuilder::default()
            .build()
            .unwrap()
    }
}

/// Open RocksDB database at given path with specified Column Family configurations
///
/// # Arguments
/// * `path` - Path to open RocksDB
/// * `cfs` - Iterator of Column Family descriptors
pub fn open_kv<P, I>(path: P, cfs: I, cfg: &DbConfiguration) -> Result<DB, DBError>
    where
        P: AsRef<Path>,
        I: IntoIterator<Item=ColumnFamilyDescriptor>,
{
    DB::open_cf_descriptors(&default_kv_options(cfg), path, cfs)
        .map_err(DBError::from)
}

/// Create default database configuration options,
/// based on recommended setting: https://github.com/facebook/rocksdb/wiki/Setup-Options-and-Basic-Tuning#other-general-options
fn default_kv_options(cfg: &DbConfiguration) -> Options {
    // default db options
    let mut db_opts = Options::default();
    db_opts.create_missing_column_families(true);
    db_opts.create_if_missing(true);

    // https://github.com/facebook/rocksdb/wiki/Setup-Options-and-Basic-Tuning#other-general-options
    db_opts.set_bytes_per_sync(1048576);
    db_opts.set_level_compaction_dynamic_level_bytes(true);
    db_opts.set_max_background_compactions(4);
    db_opts.set_max_background_flushes(2);

    // resolve thread count to use
    let num_of_threads = match cfg.max_threads {
        Some(num) => std::cmp::min(num, num_cpus::get()),
        None => num_cpus::get()
    };
    // rocksdb default is 1, so we increase only, if above 1
    if num_of_threads > 1 {
        db_opts.increase_parallelism(num_of_threads as i32);
    }

    db_opts
}

/// Create default database configuration options,
/// based on recommended setting:
///     https://github.com/facebook/rocksdb/wiki/Setup-Options-and-Basic-Tuning#other-general-options
///     https://rocksdb.org/blog/2019/03/08/format-version-4.html
pub fn default_table_options() -> Options {
    // default db options
    let mut db_opts = Options::default();

    // https://github.com/facebook/rocksdb/wiki/Setup-Options-and-Basic-Tuning#other-general-options
    db_opts.set_level_compaction_dynamic_level_bytes(true);

    // block table options
    let mut table_options = BlockBasedOptions::default();
    table_options.set_block_size(16 * 1024);
    table_options.set_cache_index_and_filter_blocks(true);
    table_options.set_pin_l0_filter_and_index_blocks_in_cache(true);

    // set format_version 4 https://rocksdb.org/blog/2019/03/08/format-version-4.html
    table_options.set_format_version(4);
    table_options.set_block_restart_interval(16);

    db_opts.set_block_based_table_factory(&table_options);

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


pub type ContextMap = BTreeMap<String, Bucket<Vec<u8>>>;
pub type ContextList = Arc<RwLock<dyn TypedSkipList<String, Bucket<Vec<u8>>> + Sync + Send>>;

/// Groups all components required for correct permanent storage functioning
#[derive(Clone)]
pub struct PersistentStorage {
    /// key-value store
    kv: Arc<DB>,
    /// commit log store
    clog: Arc<CommitLogs>,
    /// autoincrement  id generators
    seq: Arc<Sequences>,
    /// skip list backed context storage
    cs: ContextList,
    /// merkle-tree based context storage
    merkle: Arc<RwLock<MerkleStorage>>,
}

impl PersistentStorage {
    pub fn new(kv: Arc<DB>, clog: Arc<CommitLogs>) -> Self {
        let seq = Arc::new(Sequences::new(kv.clone(), 1000));
        let db = MerkleStorage::get_db("_merkle_db_test");
        Self {
            clog,
            kv: kv.clone(),
            cs: Arc::new(RwLock::new(DatabaseBackedSkipList::new(0, kv, seq.generator("skip_list")).expect("failed to initialize context storage"))),
            seq,
            merkle: Arc::new(RwLock::new(MerkleStorage::new(db))),
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

    #[inline]
    pub fn merkle(&self) -> Arc<RwLock<MerkleStorage>> {
        self.merkle.clone()
    }

    #[inline]
    pub fn context_storage(&self) -> ContextList { self.cs.clone() }
}

impl Drop for PersistentStorage {
    fn drop(&mut self) {
        self.clog.flush().expect("Failed to flush commit logs");
        self.kv.flush().expect("Failed to flush database");
    }
}
