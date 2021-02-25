// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Fail;
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::action_file::ActionFileError;
use crate::KeyValueStoreBackend;
use crate::StorageError;
use derive_builder::Builder;
use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, Options, DB};

pub use codec::{BincodeEncoded, Codec, Decoder, Encoder, SchemaError};
pub use commit_log::{CommitLogError, CommitLogRef, CommitLogWithSchema, CommitLogs, Location};
pub use database::{DBError, KeyValueStoreWithSchema, KeyValueStoreWithSchemaIterator};
pub use schema::{CommitLogDescriptor, CommitLogSchema, KeyValueSchema};

use crate::backend::btree_map::BTreeMapBackend;
use crate::backend::in_memory_backend::InMemoryBackend;
use crate::backend::mark_move_gced::MarkMoveGCed;
use crate::backend::mark_sweep_gced::MarkSweepGCed;
use crate::backend::rocksdb_backend::RocksDBBackend;
use crate::backend::sled_backend::SledBackend;
use crate::merkle_storage::MerkleStorage;
use crate::persistent::sequence::Sequences;
use tezos_context::channel::ContextAction;

pub mod codec;
pub mod commit_log;
pub mod database;
pub mod schema;
pub mod sequence;

const PRESERVE_CYCLE_COUNT: usize = 7;

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
    /// key-value store for operational database
    db: Arc<DB>,
    /// key-value store for context (used by merkle)
    db_context: Arc<DB>,
    /// context actions store
    db_context_actions: Arc<DB>,
    /// commit log store for storing plain block header data
    clog: Arc<CommitLogs>,
    /// autoincrement  id generators
    seq: Arc<Sequences>,
    /// merkle-tree based context storage (uses db_context)
    merkle: Arc<RwLock<MerkleStorage>>,
}

pub enum StorageType {
    Database,
    Context,
    ContextAction,
}

impl PersistentStorage {
    pub fn new(
        db: Arc<DB>,
        db_context: Arc<DB>,
        db_context_actions: Arc<DB>,
        clog: Arc<CommitLogs>,
        merkle_backend: KeyValueStoreBackend,
    ) -> Self {
        let merkle =
            match merkle_backend {
                KeyValueStoreBackend::RocksDB => {
                    MerkleStorage::new(Box::new(RocksDBBackend::new(db_context.clone())))
                }
                KeyValueStoreBackend::InMem => MerkleStorage::new(Box::new(InMemoryBackend::new())),
                KeyValueStoreBackend::Sled { path } => {
                    let sled = sled::Config::new().path(path).open().unwrap();
                    MerkleStorage::new(Box::new(SledBackend::new(sled)))
                }
                KeyValueStoreBackend::BTreeMap => {
                    MerkleStorage::new(Box::new(BTreeMapBackend::new()))
                }
                KeyValueStoreBackend::MarkSweepInMem => MerkleStorage::new(Box::new(
                    MarkSweepGCed::<InMemoryBackend>::new(PRESERVE_CYCLE_COUNT),
                )),
                KeyValueStoreBackend::MarkMoveInMem => MerkleStorage::new(Box::new(
                    MarkMoveGCed::<BTreeMapBackend>::new(PRESERVE_CYCLE_COUNT),
                )),
            };

        let seq = Arc::new(Sequences::new(db.clone(), 1000));
        Self {
            clog,
            db,
            db_context,
            db_context_actions,
            seq,
            merkle: Arc::new(RwLock::new(merkle)),
        }
    }

    #[inline]
    pub fn kv(&self, storage: StorageType) -> Arc<DB> {
        match storage {
            StorageType::Context => self.db_context.clone(),
            StorageType::ContextAction => self.db_context_actions.clone(),
            StorageType::Database => self.db.clone(),
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
        let db = self.db.flush();
        let db_context = self.db_context.flush();
        let db_context_actions = self.db_context_actions.flush();

        if clog.is_err() || db.is_err() || db_context.is_err() || db_context_actions.is_err() {
            println!(
                "Failed to flush DBs. clog_err: {:?}, kv_err: {:?}, kv_context: {:?}, kv_context_actions: {:?}",
                clog, db, db_context, db_context_actions
            );
        }
    }
}

impl Drop for PersistentStorage {
    fn drop(&mut self) {
        self.flush_dbs();
    }
}

#[derive(Debug, Fail)]
pub enum ActionRecordError {
    #[fail(display = "ActionFileError Error: {}", error)]
    ActionFileError { error: ActionFileError },
    #[fail(display = "Missing actions for block {:?}.", hash)]
    MissingActions { hash: String },
}

impl From<ActionFileError> for ActionRecordError {
    fn from(error: ActionFileError) -> Self {
        ActionRecordError::ActionFileError { error }
    }
}

pub trait ActionRecorder {
    fn record(&mut self, action: &ContextAction) -> Result<(), StorageError>;
}

pub struct NoRecorder {}

impl ActionRecorder for NoRecorder {
    fn record(&mut self, _action: &ContextAction) -> Result<(), StorageError> {
        Ok(())
    }
}
