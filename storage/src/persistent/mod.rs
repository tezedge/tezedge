// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;
use std::sync::{Arc, RwLock};

use derive_builder::Builder;
use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, Options, DB};

pub use codec::{BincodeEncoded, Codec, Decoder, Encoder, SchemaError};
pub use commit_log::{CommitLogError, CommitLogRef, CommitLogWithSchema, CommitLogs, Location};
pub use database::{DBError, KeyValueStoreWithSchema};
pub use schema::{CommitLogDescriptor, CommitLogSchema, KeyValueSchema};

use crate::merkle_storage::MerkleStorage;
use crate::persistent::sequence::Sequences;

pub mod codec;
pub mod commit_log;
pub mod database;
pub mod schema;
pub mod sequence;

/// Rocksdb database system configuration
/// - [max_num_of_threads] - if not set, num of cpus is used
#[derive(Builder, Debug, Clone)]
pub struct DbConfiguration {
    #[builder(default = "None")]
    pub max_threads: Option<usize>,
}

impl Default for DbConfiguration {
    fn default() -> Self {
        DbConfigurationBuilder::default().build().unwrap()
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
    I: IntoIterator<Item = ColumnFamilyDescriptor>,
{
    DB::open_cf_descriptors(&default_kv_options(cfg), path, cfs).map_err(DBError::from)
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
    db_opts.set_max_background_jobs(6);
    db_opts.enable_statistics();
    db_opts.set_report_bg_io_stats(true);

    // resolve thread count to use
    let num_of_threads = match cfg.max_threads {
        Some(num) => std::cmp::min(num, num_cpus::get()),
        None => num_cpus::get(),
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
pub fn default_table_options(cache: &Cache) -> Options {
    // default db options
    let mut db_opts = Options::default();

    // https://github.com/facebook/rocksdb/wiki/Setup-Options-and-Basic-Tuning#other-general-options
    db_opts.set_level_compaction_dynamic_level_bytes(true);

    // block table options
    let mut table_options = BlockBasedOptions::default();
    table_options.set_block_cache(cache);
    table_options.set_block_size(16 * 1024);
    table_options.set_cache_index_and_filter_blocks(true);
    table_options.set_pin_l0_filter_and_index_blocks_in_cache(true);

    // set format_version 4 https://rocksdb.org/blog/2019/03/08/format-version-4.html
    table_options.set_format_version(4);
    table_options.set_index_block_restart_interval(16);

    db_opts.set_block_based_table_factory(&table_options);

    db_opts
}

/// Open commit log at a given path.
pub fn open_cl<P, I>(path: P, cfs: I) -> Result<CommitLogs, CommitLogError>
where
    P: AsRef<Path>,
    I: IntoIterator<Item = CommitLogDescriptor>,
{
    CommitLogs::new(path, cfs)
}

/// Groups all components required for correct permanent storage functioning
#[derive(Clone)]
pub struct PersistentStorage {
    /// key-value store
    //TODO to be renamed to kv_db
    kv: Arc<DB>,
    /// context store
    kv_context: Arc<DB>,
    /// context actions store
    kv_context_actions: Arc<DB>,
    /// commit log store
    clog: Arc<CommitLogs>,
    /// autoincrement  id generators
    seq: Arc<Sequences>,
    /// merkle-tree based context storage
    merkle: Arc<RwLock<MerkleStorage>>,
}

pub enum StorageType {
    Database,
    Context,
    ContextAction,
}

impl PersistentStorage {
    pub fn new(
        kv: Arc<DB>,
        kv_context: Arc<DB>,
        kv_context_actions: Arc<DB>,
        clog: Arc<CommitLogs>,
    ) -> Self {
        let seq = Arc::new(Sequences::new(kv.clone(), 1000));
        Self {
            clog,
            kv,
            kv_context: kv_context.clone(),
            kv_context_actions,
            seq,
            merkle: Arc::new(RwLock::new(MerkleStorage::new(kv_context))),
        }
    }

    #[inline]
    pub fn kv(&self, storage: StorageType) -> Arc<DB> {
        match storage {
            StorageType::Context => self.kv_context.clone(),
            StorageType::ContextAction => self.kv_context_actions.clone(),
            StorageType::Database => self.kv.clone(),
        }
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

    pub fn flush_dbs(&mut self) {
        let clog = self.clog.flush();
        let kv = self.kv.flush();
        if clog.is_err() || kv.is_err() {
            println!(
                "Failed to flush DBs. clog_err: {:?}, kv_err: {:?}",
                clog, kv
            );
        }
    }
}

impl Drop for PersistentStorage {
    fn drop(&mut self) {
        self.flush_dbs();
    }
}
