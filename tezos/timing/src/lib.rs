// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    cell::Cell,
    collections::HashMap,
    convert::TryInto,
    path::PathBuf,
    sync::{
        mpsc::{sync_channel, Receiver, SendError, SyncSender},
        Mutex, PoisonError,
    },
    time::{Duration, Instant},
};

use crypto::hash::{BlockHash, ContextHash, OperationHash};
use failure::Fail;
use once_cell::sync::Lazy;
use rusqlite::{named_params, Batch, Connection, Error as SQLError, Transaction};
use serde::Serialize;

pub const FILENAME_DB: &str = "context_stats.db";

#[derive(Debug)]
pub struct BlockMemoryUsage {
    pub context: Box<ContextMemoryUsage>,
    pub serialize: Box<SerializeStats>,
}

#[derive(Debug, Default)]
pub struct SerializeStats {
    pub blobs_length: usize,
    pub hash_ids_length: usize,
    pub keys_length: usize,
    pub highest_hash_id: u32,
    pub ndirectories: usize,
    pub nblobs: usize,
    pub nblobs_inlined: usize,
    pub total_bytes: usize,
}

impl SerializeStats {
    pub fn add_directory(
        &mut self,
        hash_ids_length: usize,
        keys_length: usize,
        highest_hash_id: u32,
        nblobs_inlined: usize,
        blobs_length: usize,
    ) {
        self.ndirectories += 1;
        self.hash_ids_length += hash_ids_length;
        self.keys_length += keys_length;
        self.highest_hash_id = self.highest_hash_id.max(highest_hash_id);
        self.nblobs_inlined += nblobs_inlined;
        self.blobs_length += blobs_length;
    }

    pub fn add_blob(&mut self, blob_length: usize) {
        self.nblobs += 1;
        self.blobs_length += blob_length;
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ContextMemoryUsage {
    pub repo: RepositoryMemoryUsage,
    pub storage: StorageMemoryUsage,
}

#[derive(Debug)]
pub struct StorageMemoryUsage {
    pub nodes_len: usize,
    pub nodes_cap: usize,
    pub directories_len: usize,
    pub directories_cap: usize,
    pub temp_dir_cap: usize,
    pub blobs_len: usize,
    pub blobs_cap: usize,
    pub inodes_len: usize,
    pub inodes_cap: usize,
    pub strings: StringsMemoryUsage,
    pub total_bytes: usize,
}

#[derive(Debug)]
pub struct StringsMemoryUsage {
    pub all_strings_map_cap: usize,
    pub all_strings_map_len: usize,
    pub all_strings_cap: usize,
    pub all_strings_len: usize,
    pub big_strings_cap: usize,
    pub big_strings_len: usize,
    pub big_strings_map_cap: usize,
    pub big_strings_map_len: usize,
    pub total_bytes: usize,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct RepositoryMemoryUsage {
    /// Number of bytes for all values Arc<[u8]>
    pub values_bytes: usize,
    /// Capacity of the Vec for the values
    pub values_capacity: usize,
    /// Length of the Vec for the values
    pub values_length: usize,
    /// Capacity of the Vec for the hashes
    pub hashes_capacity: usize,
    /// Capacity of the Vec for the hashes
    pub hashes_length: usize,
    /// Total bytes occupied in the repository
    pub total_bytes: usize,
}

#[derive(Debug)]
pub enum QueryKind {
    Mem,
    MemTree,
    Find,
    FindTree,
    Add,
    AddTree,
    Remove,
}

impl QueryKind {
    fn to_str(&self) -> &'static str {
        match self {
            QueryKind::Mem => "mem",
            QueryKind::MemTree => "mem_tree",
            QueryKind::Find => "find",
            QueryKind::FindTree => "find_tree",
            QueryKind::Add => "add",
            QueryKind::AddTree => "add_tree",
            QueryKind::Remove => "remove",
        }
    }
}

// TODO: add tree_action

#[derive(Debug)]
pub enum TimingMessage {
    SetBlock {
        block_hash: Option<BlockHash>,
        /// Duration since std::time::UNIX_EPOCH.
        /// It is `None` when `block_hash` is `None`.
        timestamp: Option<Duration>,
        /// Instant when the hook `set_block` was called.
        /// `Instant` is required because it's monotonic, `SystemTime` (which
        /// is used to get `timestamp`) is not.
        instant: Instant,
    },
    SetOperation(Option<OperationHash>),
    Checkout {
        context_hash: ContextHash,
        irmin_time: Option<f64>,
        tezedge_time: Option<f64>,
    },
    Commit {
        irmin_time: Option<f64>,
        tezedge_time: Option<f64>,
    },
    Query(Query),
    InitTiming {
        db_path: Option<PathBuf>,
    },
    BlockMemoryUsage {
        stats: BlockMemoryUsage,
    },
}

// Id of the hash in the database
type HashId = String;

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DetailedTime {
    pub count: usize,
    pub mean_time: f64,
    pub max_time: f64,
    pub total_time: f64,
}

impl DetailedTime {
    fn compute_mean(&mut self) {
        let mean = self.total_time / self.count as f64;
        self.mean_time = mean.max(0.0);
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RangeStats {
    pub total_time: f64,
    pub queries_count: usize,
    pub one_to_ten_us: DetailedTime,
    pub ten_to_one_hundred_us: DetailedTime,
    pub one_hundred_us_to_one_ms: DetailedTime,
    pub one_to_ten_ms: DetailedTime,
    pub ten_to_one_hundred_ms: DetailedTime,
    pub one_hundred_ms_to_one_s: DetailedTime,
    pub one_to_ten_s: DetailedTime,
    pub ten_to_one_hundred_s: DetailedTime,
    pub one_hundred_s: DetailedTime,
}

impl RangeStats {
    fn compute_mean(&mut self) {
        self.one_to_ten_us.compute_mean();
        self.ten_to_one_hundred_us.compute_mean();
        self.one_hundred_us_to_one_ms.compute_mean();
        self.one_to_ten_ms.compute_mean();
        self.ten_to_one_hundred_ms.compute_mean();
        self.one_hundred_ms_to_one_s.compute_mean();
        self.one_to_ten_s.compute_mean();
        self.ten_to_one_hundred_s.compute_mean();
        self.one_hundred_s.compute_mean();
    }

    fn add_time<T: Into<Option<f64>>>(&mut self, time: T) {
        let time = match time.into() {
            Some(t) => t,
            None => return,
        };

        self.total_time += time;
        self.queries_count = self.queries_count.saturating_add(1);

        let entry = match time {
            t if t < 0.00001 => &mut self.one_to_ten_us,
            t if t < 0.0001 => &mut self.ten_to_one_hundred_us,
            t if t < 0.001 => &mut self.one_hundred_us_to_one_ms,
            t if t < 0.01 => &mut self.one_to_ten_ms,
            t if t < 0.1 => &mut self.ten_to_one_hundred_ms,
            t if t < 1.0 => &mut self.one_hundred_ms_to_one_s,
            t if t < 10.0 => &mut self.one_to_ten_s,
            t if t < 100.0 => &mut self.ten_to_one_hundred_s,
            _ => &mut self.one_hundred_s,
        };
        entry.count = entry.count.saturating_add(1);
        entry.total_time += time;
        entry.max_time = entry.max_time.max(time);
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryStatsWithRange {
    pub root: String,
    pub total_time_read: f64,
    pub total_time_write: f64,
    pub total_time: f64,
    pub queries_count: usize,
    pub mem: RangeStats,
    pub mem_tree: RangeStats,
    pub find: RangeStats,
    pub find_tree: RangeStats,
    pub add: RangeStats,
    pub add_tree: RangeStats,
    pub remove: RangeStats,
}

impl QueryStatsWithRange {
    fn compute_mean(&mut self) {
        self.mem.compute_mean();
        self.mem_tree.compute_mean();
        self.find.compute_mean();
        self.find_tree.compute_mean();
        self.add.compute_mean();
        self.add_tree.compute_mean();
        self.remove.compute_mean();
    }

    pub fn compute_total(&mut self) {
        self.total_time_read = self.mem.total_time
            + self.mem_tree.total_time
            + self.find.total_time
            + self.find_tree.total_time;

        self.total_time_write =
            self.add.total_time + self.add_tree.total_time + self.remove.total_time;

        self.total_time = self.total_time_read + self.total_time_write;

        self.queries_count = self
            .mem
            .queries_count
            .saturating_add(self.mem_tree.queries_count)
            .saturating_add(self.find.queries_count)
            .saturating_add(self.find_tree.queries_count)
            .saturating_add(self.add.queries_count)
            .saturating_add(self.add_tree.queries_count)
            .saturating_add(self.remove.queries_count);
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryData {
    pub root: String,
    pub tezedge_count: usize,
    pub irmin_count: usize,
    pub tezedge_mean_time: f64,
    pub tezedge_max_time: f64,
    pub tezedge_total_time: f64,
    pub irmin_mean_time: f64,
    pub irmin_max_time: f64,
    pub irmin_total_time: f64,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryStats {
    pub data: QueryData,
    pub tezedge_mem: f64,
    pub tezedge_mem_tree: f64,
    pub tezedge_find: f64,
    pub tezedge_find_tree: f64,
    pub tezedge_add: f64,
    pub tezedge_add_tree: f64,
    pub tezedge_remove: f64,
    pub irmin_mem: f64,
    pub irmin_mem_tree: f64,
    pub irmin_find: f64,
    pub irmin_find_tree: f64,
    pub irmin_add: f64,
    pub irmin_add_tree: f64,
    pub irmin_remove: f64,
}

impl QueryStats {
    fn compute_mean(&mut self) {
        let mean = self.data.tezedge_total_time / self.data.tezedge_count as f64;
        self.data.tezedge_mean_time = mean.max(0.0);

        let mean = self.data.irmin_total_time / self.data.irmin_count as f64;
        self.data.irmin_mean_time = mean.max(0.0);
    }
}

struct Timing {
    current_block: Option<(HashId, BlockHash)>,
    current_operation: Option<(HashId, OperationHash)>,
    current_context: Option<(HashId, ContextHash)>,
    block_started_at: Option<(Duration, Instant)>,
    /// Number of queries in current block
    nqueries: usize,
    /// Checkout time for the current block
    checkout_time: Option<(Option<f64>, Option<f64>)>,
    /// Statistics for the current block
    block_stats: HashMap<String, QueryStats>,
    /// Global statistics
    tezedge_global_stats: HashMap<String, QueryStatsWithRange>,
    irmin_global_stats: HashMap<String, QueryStatsWithRange>,
    tezedge_commit_stats: RangeStats,
    irmin_commit_stats: RangeStats,
    tezedge_checkout_stats: RangeStats,
    irmin_checkout_stats: RangeStats,
}

impl std::fmt::Debug for Timing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timing")
            .field("current_block", &self.current_block)
            .field("current_operation", &self.current_operation)
            .field("current_context", &self.current_context)
            .finish()
    }
}

#[derive(Debug)]
pub struct Query {
    pub query_name: QueryKind,
    pub key: Vec<String>,
    pub irmin_time: Option<f64>,
    pub tezedge_time: Option<f64>,
}

#[derive(Fail, Debug)]
pub enum BufferedTimingChannelSendError {
    #[fail(
        display = "Failure when locking the timings channel buffer: {}",
        reason
    )]
    LockError { reason: String },
    #[fail(
        display = "Failure when sending timming messages to channel: {}",
        reason
    )]
    SendError {
        reason: SendError<Vec<TimingMessage>>,
    },
}

impl From<SendError<Vec<TimingMessage>>> for BufferedTimingChannelSendError {
    fn from(reason: SendError<Vec<TimingMessage>>) -> Self {
        Self::SendError { reason }
    }
}

impl<T> From<PoisonError<T>> for BufferedTimingChannelSendError {
    fn from(reason: PoisonError<T>) -> Self {
        Self::LockError {
            reason: format!("{}", reason),
        }
    }
}

/// Buffered channel for sending timings that delays the sending until
/// enough messages have been obtained or a commit message is received.
/// The purpose is to send less messages through the channel to decrease
/// the overhead.
pub struct BufferedTimingChannel {
    buffer: Mutex<Cell<Vec<TimingMessage>>>,
    sender: SyncSender<Vec<TimingMessage>>,
}

impl BufferedTimingChannel {
    const DELAYED_MESSAGES_LIMIT: usize = 100;

    fn new(sender: SyncSender<Vec<TimingMessage>>) -> Self {
        Self {
            buffer: Mutex::new(Cell::new(Vec::with_capacity(Self::DELAYED_MESSAGES_LIMIT))),
            sender,
        }
    }

    /// True if the message must be sent immediately, false if it can be buffered.
    fn is_immediate_message(&self, msg: &TimingMessage) -> bool {
        match msg {
            TimingMessage::Commit { .. } => true,
            TimingMessage::InitTiming { .. } => false,
            _ => false,
        }
    }

    /// Sends messages, delayed and combined into a single bigger message.
    ///
    /// Reaching the delayed messages limit or receiving a commit message will trigger the send
    /// to the underlying channel, otherwise the messages will be kept in the buffer.
    pub fn send(&self, msg: TimingMessage) -> Result<(), BufferedTimingChannelSendError> {
        let must_not_delay = self.is_immediate_message(&msg);
        let limit = Self::DELAYED_MESSAGES_LIMIT - 1;
        let mut buffer = self.buffer.lock()?;

        buffer.get_mut().push(msg);

        if must_not_delay || buffer.get_mut().len() == limit {
            let swap_buffer = Cell::new(Vec::with_capacity(Self::DELAYED_MESSAGES_LIMIT));

            buffer.swap(&swap_buffer);

            let pack = swap_buffer.into_inner();

            Ok(self.sender.send(pack)?)
        } else {
            Ok(())
        }
    }
}

pub static TIMING_CHANNEL: Lazy<BufferedTimingChannel> = Lazy::new(|| {
    let (sender, receiver) = sync_channel(10_000);

    if let Err(e) = std::thread::Builder::new()
        .name("ctx-timings-thread".to_string())
        .spawn(|| {
            start_timing(receiver);
        })
    {
        eprintln!("Fail to create timing channel: {:?}", e);
    }

    BufferedTimingChannel::new(sender)
});

fn start_timing(recv: Receiver<Vec<TimingMessage>>) {
    let mut db_path: Option<PathBuf> = None;

    'outer: for msgpack in &recv {
        for msg in msgpack {
            if let TimingMessage::InitTiming { db_path: path } = msg {
                db_path = path;
                break 'outer;
            };
        }
    }

    let sql = match Timing::init_sqlite(db_path) {
        Ok(sql) => sql,
        Err(e) => {
            eprintln!("Fail to initialize timing {:?}", e);
            return;
        }
    };

    let mut timing = Timing::new();
    let mut transaction = None;

    for msgpack in recv {
        for msg in msgpack {
            if let Err(err) = timing.process_msg(&sql, &mut transaction, msg) {
                eprintln!("Timing error={:?}", err);
            }
        }
    }
}

pub fn hash_to_string(hash: &[u8]) -> String {
    const HEXCHARS: &[u8] = b"0123456789abcdef";

    let mut s = String::with_capacity(62);
    for byte in hash {
        s.push(HEXCHARS[*byte as usize >> 4] as char);
        s.push(HEXCHARS[*byte as usize & 0xF] as char);
    }
    s
}

impl Timing {
    fn new() -> Timing {
        Timing {
            current_block: None,
            current_operation: None,
            current_context: None,
            block_started_at: None,
            nqueries: 0,
            checkout_time: None,
            block_stats: HashMap::default(),
            tezedge_global_stats: HashMap::default(),
            irmin_global_stats: HashMap::default(),
            tezedge_commit_stats: Default::default(),
            irmin_commit_stats: RangeStats::default(),
            tezedge_checkout_stats: Default::default(),
            irmin_checkout_stats: RangeStats::default(),
        }
    }

    fn process_msg<'a>(
        &mut self,
        sql: &'a Connection,
        transaction: &mut Option<Transaction<'a>>,
        msg: TimingMessage,
    ) -> Result<(), SQLError> {
        match msg {
            TimingMessage::SetBlock {
                block_hash,
                timestamp,
                instant,
            } => self.set_current_block(sql, block_hash, timestamp, instant, transaction),
            TimingMessage::SetOperation(operation_hash) => {
                self.set_current_operation(sql, operation_hash)
            }
            TimingMessage::Query(query) => self.insert_query(sql, transaction, &query),
            TimingMessage::Checkout {
                context_hash,
                irmin_time,
                tezedge_time,
            } => self.insert_checkout(sql, context_hash, irmin_time, tezedge_time),
            TimingMessage::Commit {
                irmin_time,
                tezedge_time,
            } => self.insert_commit(sql, irmin_time, tezedge_time),
            TimingMessage::BlockMemoryUsage { stats } => self.insert_block_memory_usage(sql, stats),
            TimingMessage::InitTiming { .. } => Ok(()),
        }
    }

    fn insert_block_memory_usage<'a>(
        &mut self,
        sql: &'a Connection,
        stats: BlockMemoryUsage,
    ) -> Result<(), SQLError> {
        let block_id = match self.current_block.as_ref().map(|b| b.0.as_str()) {
            Some(block_id) => block_id,
            None => return Ok(()),
        };

        let mut query = sql.prepare_cached(
            "
            UPDATE
              blocks
            SET
              repo_values_bytes = :repo_values_bytes,
              repo_values_capacity = :repo_values_capacity,
              repo_values_length = :repo_values_length,
              repo_hashes_capacity = :repo_hashes_capacity,
              repo_hashes_length = :repo_hashes_length,
              repo_total_bytes = :repo_total_bytes,
              storage_nodes_capacity = :storage_nodes_capacity,
              storage_nodes_length = :storage_nodes_length,
              storage_trees_capacity = :storage_trees_capacity,
              storage_trees_length = :storage_trees_length,
              storage_temp_tree_capacity = :storage_temp_tree_capacity,
              storage_blobs_capacity = :storage_blobs_capacity,
              storage_blobs_length = :storage_blobs_length,
              storage_strings_capacity = :storage_strings_capacity,
              storage_strings_length = :storage_strings_length,
              storage_strings_map_capacity = :storage_strings_map_capacity,
              storage_strings_map_length = :storage_strings_map_length,
              storage_big_strings_capacity = :storage_big_strings_capacity,
              storage_big_strings_length = :storage_big_strings_length,
              storage_big_strings_map_capacity = :storage_big_strings_map_capacity,
              storage_big_strings_map_length = :storage_big_strings_map_length,
              storage_total_bytes = :storage_total_bytes,
              storage_strings_total_bytes = :storage_strings_total_bytes,
              serialize_blobs_length = :serialize_blobs_length,
              serialize_hashids_length = :serialize_hashids_length,
              serialize_keys_length = :serialize_keys_length,
              serialize_highest_hash_id_length = :serialize_highest_hash_id_length,
              serialize_ntree = :serialize_ntree,
              serialize_nblobs = :serialize_nblobs,
              serialize_nblobs_inlined = :serialize_nblobs_inlined,
              serialize_total_bytes = :serialize_total_bytes,
              total_bytes = :total_bytes
            WHERE
              id = :block_id;
                ",
        )?;

        query.execute(named_params! {
            ":repo_values_bytes": stats.context.repo.values_bytes,
            ":repo_values_capacity": stats.context.repo.values_capacity,
            ":repo_values_length": stats.context.repo.values_length,
            ":repo_hashes_capacity": stats.context.repo.hashes_capacity,
            ":repo_hashes_length": stats.context.repo.hashes_length,
            ":repo_total_bytes": stats.context.repo.total_bytes,
            ":storage_nodes_length": stats.context.storage.nodes_len,
            ":storage_nodes_capacity": stats.context.storage.nodes_cap,
            ":storage_trees_length": stats.context.storage.directories_len,
            ":storage_trees_capacity": stats.context.storage.directories_cap,
            ":storage_temp_tree_capacity": stats.context.storage.temp_dir_cap,
            ":storage_blobs_length": stats.context.storage.blobs_len,
            ":storage_blobs_capacity": stats.context.storage.blobs_cap,
            ":storage_strings_length": stats.context.storage.strings.all_strings_len,
            ":storage_strings_capacity": stats.context.storage.strings.all_strings_cap,
            ":storage_strings_map_length": stats.context.storage.strings.all_strings_map_len,
            ":storage_strings_map_capacity": stats.context.storage.strings.all_strings_map_cap,
            ":storage_big_strings_length": stats.context.storage.strings.big_strings_len,
            ":storage_big_strings_capacity": stats.context.storage.strings.big_strings_cap,
            ":storage_big_strings_map_length": stats.context.storage.strings.big_strings_map_len,
            ":storage_big_strings_map_capacity": stats.context.storage.strings.big_strings_map_cap,
            ":storage_total_bytes": stats.context.storage.total_bytes,
            ":storage_strings_total_bytes": stats.context.storage.strings.total_bytes,
            ":serialize_blobs_length": stats.serialize.blobs_length,
            ":serialize_hashids_length": stats.serialize.hash_ids_length,
            ":serialize_keys_length": stats.serialize.keys_length,
            ":serialize_highest_hash_id_length": stats.serialize.highest_hash_id,
            ":serialize_ntree": stats.serialize.ndirectories,
            ":serialize_nblobs": stats.serialize.nblobs,
            ":serialize_nblobs_inlined": stats.serialize.nblobs_inlined,
            ":serialize_total_bytes": stats.serialize.total_bytes,
            ":total_bytes": stats.context.repo.total_bytes
                .saturating_add(stats.context.storage.total_bytes)
                .saturating_add(stats.context.storage.strings.total_bytes),
            ":block_id": block_id,
        })?;

        Ok(())
    }

    fn set_current_block<'a>(
        &mut self,
        sql: &'a Connection,
        block_hash: Option<BlockHash>,
        mut timestamp: Option<Duration>,
        instant: Instant,
        transaction: &mut Option<Transaction<'a>>,
    ) -> Result<(), SQLError> {
        if let Some(timestamp) = timestamp.take() {
            self.block_started_at = Some((timestamp, instant));
        } else if let Some(started_at) = self.block_started_at.take() {
            let started_at_instant = started_at.1;
            let started_at_timestamp = started_at.0;

            let duration_millis: u64 = instant
                .saturating_duration_since(started_at_instant)
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX);

            let timestamp_secs = started_at_timestamp.as_secs();
            let timestamp_nanos = started_at_timestamp.subsec_nanos();
            let block_id = self.current_block.as_ref().map(|b| b.0.as_str());

            let mut query = sql.prepare_cached(
                "
            UPDATE
              blocks
            SET
              timestamp_secs = :timestamp_secs,
              timestamp_nanos = :timestamp_nanos,
              duration_millis = :duration
            WHERE
              id = :block_id;
                ",
            )?;

            query.execute(named_params! {
                ":timestamp_secs": timestamp_secs,
                ":timestamp_nanos": timestamp_nanos,
                ":duration": duration_millis,
                ":block_id": block_id,
            })?;
        }

        Self::set_current(sql, block_hash, &mut self.current_block, "blocks")?;

        if let Some(transaction) = transaction.take() {
            transaction.commit()?;
        };
        *transaction = Some(sql.unchecked_transaction()?);

        // Reset context and operation
        self.current_context = None;
        self.current_operation = None;
        self.checkout_time = None;
        self.nqueries = 0;
        self.block_stats = HashMap::default();

        Ok(())
    }

    fn set_current_operation(
        &mut self,
        sql: &Connection,
        operation_hash: Option<OperationHash>,
    ) -> Result<(), SQLError> {
        Self::set_current(
            sql,
            operation_hash,
            &mut self.current_operation,
            "operations",
        )
    }

    fn set_current_context(
        &mut self,
        sql: &Connection,
        context_hash: ContextHash,
    ) -> Result<(), SQLError> {
        Self::set_current(
            sql,
            Some(context_hash),
            &mut self.current_context,
            "contexts",
        )
    }

    fn set_current<T>(
        sql: &Connection,
        hash: Option<T>,
        current: &mut Option<(HashId, T)>,
        table_name: &str,
    ) -> Result<(), SQLError>
    where
        T: Eq,
        T: AsRef<Vec<u8>>,
    {
        match (hash.as_ref(), current.as_ref()) {
            (None, _) => {
                *current = None;
                return Ok(());
            }
            (Some(hash), Some((_, current_hash))) if hash == current_hash => {
                return Ok(());
            }
            _ => {}
        };

        let hash = hash.unwrap(); // `hash` is never None, it would have matched on the statement above
        let hash_string = hash_to_string(hash.as_ref());

        if let Some(id) = Self::get_id_on_table(sql, table_name, &hash_string)? {
            current.replace((id.to_string(), hash));
            return Ok(());
        };

        // FIXME: this "OR IGNORE" is only valid for checkouts, if we get a duplicate when doing something
        // else there may be a problem.
        // Related to this, in general we want to avoid except/unwrap, because this will kill the thread and
        // everything will stop working, it should be handled more gracefully if possible.
        // Not a priority right now but we have to think about how to properly handle such situations, specially
        // if this functionality gets extended to include more data about the context queries.
        sql.execute(
            &format!(
                "INSERT OR IGNORE INTO {table} (hash) VALUES (?1);",
                table = table_name
            ),
            [&hash_string],
        )?;

        let id = Self::get_id_on_table(sql, table_name, &hash_string)?
            .expect("Unable to find row after INSERT"); // This should never happen

        current.replace((id.to_string(), hash));

        Ok(())
    }

    fn get_id_on_table(
        sql: &Connection,
        table_name: &str,
        hash_string: &str,
    ) -> Result<Option<i64>, SQLError> {
        let mut stmt = sql.prepare(&format!(
            "SELECT id FROM {table} WHERE hash = ?1;",
            table = table_name
        ))?;
        let mut rows = stmt.query([hash_string])?;

        if let Some(row) = rows.next()? {
            return Ok(Some(row.get(0)?));
        };

        Ok(None)
    }

    fn insert_checkout(
        &mut self,
        sql: &Connection,
        context_hash: ContextHash,
        irmin_time: Option<f64>,
        tezedge_time: Option<f64>,
    ) -> Result<(), SQLError> {
        if self.current_block.is_none() {
            return Ok(());
        }

        self.tezedge_checkout_stats.add_time(tezedge_time);
        self.irmin_checkout_stats.add_time(irmin_time);
        self.set_current_context(sql, context_hash)?;
        self.checkout_time = Some((irmin_time, tezedge_time));

        Ok(())
    }

    fn insert_commit(
        &mut self,
        sql: &Connection,
        irmin_time: Option<f64>,
        tezedge_time: Option<f64>,
    ) -> Result<(), SQLError> {
        if self.current_block.is_none() {
            return Ok(());
        }

        self.tezedge_commit_stats.add_time(tezedge_time);
        self.irmin_commit_stats.add_time(irmin_time);
        self.sync_global_stats(sql, irmin_time, tezedge_time)?;
        self.sync_block_stats(sql)?;

        Ok(())
    }

    fn insert_query<'a>(
        &mut self,
        sql: &'a Connection,
        transaction: &mut Option<Transaction<'a>>,
        query: &Query,
    ) -> Result<(), SQLError> {
        if self.current_block.is_none() {
            return Ok(());
        }

        let block_id = self.current_block.as_ref().map(|(id, _)| id.as_str());
        let operation_id = self.current_operation.as_ref().map(|(id, _)| id.as_str());
        let context_id = self.current_context.as_ref().map(|(id, _)| id.as_str());
        let query_name = query.query_name.to_str();

        let (root, key_id) = if query.key.is_empty() {
            (None, None)
        } else {
            let root = query.key[0].as_str();
            let key = query.key.join("/");

            let mut stmt = sql.prepare_cached("INSERT OR IGNORE INTO keys (key) VALUES (?1)")?;

            stmt.execute([key.as_str()])?;

            let mut stmt = sql.prepare_cached("SELECT id FROM keys WHERE key = ?1;")?;

            let key_id: usize = stmt.query_row([key.as_str()], |row| row.get(0))?;

            (Some(root), Some(key_id))
        };

        // TODO - TE-261: disabled for now because it is not used for anything
        // Re-enable once needed, and maybe add a command-line flag for it.
        // We probably want to also add some kind of garbage collection to only keep
        // values for the last N cycles, and not everything since the beginning.
        if false {
            if let Some(transaction) = transaction.as_ref() {
                let mut stmt = transaction.prepare_cached(
                    "
        INSERT INTO queries
          (name, key_root, key_id, irmin_time, tezedge_time, block_id, operation_id, context_id)
        VALUES
          (:name, :key_root, :key_id, :irmin_time, :tezedge_time, :block_id, :operation_id, :context_id);
                    "
                )?;

                stmt.execute(named_params! {
                    ":name": query_name,
                    ":key_root": &root,
                    ":key_id": &key_id,
                    ":irmin_time": &query.irmin_time,
                    ":tezedge_time": &query.tezedge_time,
                    ":block_id": block_id,
                    ":operation_id": operation_id,
                    ":context_id": context_id
                })?;

                drop(stmt);
            } else {
                eprintln!("Timing cannot insert query: missing `transaction`");
            }
        }

        self.nqueries = self.nqueries.saturating_add(1);

        let root = match root {
            Some(root) => root,
            None => return Ok(()),
        };

        self.add_block_stats(root, &query);
        self.add_global_stats(root, &query);

        Ok(())
    }

    fn add_global_stats(&mut self, root: &str, query: &Query) {
        for (global_stats, time) in &mut [
            (&mut self.tezedge_global_stats, query.tezedge_time),
            (&mut self.irmin_global_stats, query.irmin_time),
        ] {
            let time = match time {
                Some(time) => time,
                None => continue,
            };

            let entry = match global_stats.get_mut(root) {
                Some(entry) => entry,
                None => {
                    let stats = QueryStatsWithRange {
                        root: root.to_string(),
                        ..Default::default()
                    };
                    global_stats.insert(root.to_string(), stats);
                    match global_stats.get_mut(root) {
                        Some(entry) => entry,
                        None => {
                            eprintln!("Fail to get timing entry {:?}", root);
                            continue;
                        }
                    }
                }
            };

            let time = *time;
            let query_stats = match query.query_name {
                QueryKind::Mem => &mut entry.mem,
                QueryKind::MemTree => &mut entry.mem_tree,
                QueryKind::Find => &mut entry.find,
                QueryKind::FindTree => &mut entry.find_tree,
                QueryKind::Add => &mut entry.add,
                QueryKind::AddTree => &mut entry.add_tree,
                QueryKind::Remove => &mut entry.remove,
            };

            entry.queries_count = entry.queries_count.saturating_add(1);
            entry.total_time += time;
            query_stats.add_time(time);
        }
    }

    fn add_block_stats(&mut self, root: &str, query: &Query) {
        let entry = match self.block_stats.get_mut(root) {
            Some(entry) => entry,
            None => {
                let mut stats = QueryStats::default();
                stats.data.root = root.to_string();
                self.block_stats.insert(root.to_string(), stats);
                match self.block_stats.get_mut(root) {
                    Some(entry) => entry,
                    None => {
                        eprintln!("Fail to get timing entry {:?}", root);
                        return;
                    }
                }
            }
        };

        let (value_tezedge, value_irmin) = match query.query_name {
            QueryKind::Mem => (&mut entry.tezedge_mem, &mut entry.irmin_mem),
            QueryKind::MemTree => (&mut entry.tezedge_mem_tree, &mut entry.irmin_mem_tree),
            QueryKind::Find => (&mut entry.tezedge_find, &mut entry.irmin_find),
            QueryKind::FindTree => (&mut entry.tezedge_find_tree, &mut entry.irmin_find_tree),
            QueryKind::Add => (&mut entry.tezedge_add, &mut entry.irmin_add),
            QueryKind::AddTree => (&mut entry.tezedge_add_tree, &mut entry.irmin_add_tree),
            QueryKind::Remove => (&mut entry.tezedge_remove, &mut entry.irmin_remove),
        };

        if let Some(time) = query.tezedge_time {
            *value_tezedge += time;
            entry.data.tezedge_count = entry.data.tezedge_count.saturating_add(1);
            entry.data.tezedge_total_time += time;
            entry.data.tezedge_max_time = entry.data.tezedge_max_time.max(time);
        };

        if let Some(time) = query.irmin_time {
            *value_irmin += time;
            entry.data.irmin_count = entry.data.irmin_count.saturating_add(1);
            entry.data.irmin_total_time += time;
            entry.data.irmin_max_time = entry.data.irmin_max_time.max(time);
        };
    }

    fn sync_block_stats(&mut self, sql: &Connection) -> Result<(), SQLError> {
        for query in self.block_stats.values_mut() {
            query.compute_mean();
        }

        let block_id = self.current_block.as_ref().map(|(id, _)| id.as_str());

        for (root, query_stats) in self.block_stats.iter() {
            let root = root.as_str();

            let mut query = sql.prepare_cached(
                "
            INSERT INTO block_query_stats
              (root, block_id, tezedge_count, irmin_count,
               tezedge_mean_time, tezedge_max_time, tezedge_total_time, tezedge_mem_time, tezedge_mem_tree_time, tezedge_find_time,
               tezedge_find_tree_time, tezedge_add_time, tezedge_add_tree_time, tezedge_remove_time,
               irmin_mean_time, irmin_max_time, irmin_total_time, irmin_mem_time, irmin_mem_tree_time, irmin_find_time,
               irmin_find_tree_time, irmin_add_time, irmin_add_tree_time, irmin_remove_time)
            VALUES
              (:root, :block_id, :tezedge_count, :irmin_count,
               :tezedge_mean_time, :tezedge_max_time, :tezedge_total_time, :tezedge_mem_time, :tezedge_mem_tree_time, :tezedge_find_time,
               :tezedge_find_tree_time, :tezedge_add_time, :tezedge_add_tree_time, :tezedge_remove_time,
               :irmin_mean_time, :irmin_max_time, :irmin_total_time, :irmin_mem_time, :irmin_mem_tree_time, :irmin_find_time,
               :irmin_find_tree_time, :irmin_add_time, :irmin_add_tree_time, :irmin_remove_time)
                ",
            )?;

            query.execute(named_params! {
                ":root": root,
                ":block_id": block_id,
                ":tezedge_count": query_stats.data.tezedge_count,
                ":irmin_count": query_stats.data.irmin_count,
                ":tezedge_mean_time": query_stats.data.tezedge_mean_time,
                ":tezedge_max_time": query_stats.data.tezedge_max_time,
                ":tezedge_total_time": query_stats.data.tezedge_total_time,
                ":irmin_mean_time": query_stats.data.irmin_mean_time,
                ":irmin_max_time": query_stats.data.irmin_max_time,
                ":irmin_total_time": query_stats.data.irmin_total_time,
                ":tezedge_mem_time": query_stats.tezedge_mem,
                ":tezedge_mem_tree_time": query_stats.tezedge_mem_tree,
                ":tezedge_add_time": query_stats.tezedge_add,
                ":tezedge_add_tree_time": query_stats.tezedge_add_tree,
                ":tezedge_find_time": query_stats.tezedge_find,
                ":tezedge_find_tree_time": query_stats.tezedge_find_tree,
                ":tezedge_remove_time": query_stats.tezedge_remove,
                ":irmin_mem_time": query_stats.irmin_mem,
                ":irmin_mem_tree_time": query_stats.irmin_mem_tree,
                ":irmin_add_time": query_stats.irmin_add,
                ":irmin_add_tree_time": query_stats.irmin_add_tree,
                ":irmin_find_time": query_stats.irmin_find,
                ":irmin_find_tree_time": query_stats.irmin_find_tree,
                ":irmin_remove_time": query_stats.irmin_remove,
            })?;
        }

        Ok(())
    }

    // Compute stats for the current block and global ones
    fn sync_global_stats(
        &mut self,
        sql: &Connection,
        commit_time_irmin: Option<f64>,
        commit_time_tezedge: Option<f64>,
    ) -> Result<(), SQLError> {
        let block_id = self.current_block.as_ref().map(|(id, _)| id.as_str());

        let mut query = sql.prepare_cached(
            "
        UPDATE
          blocks
        SET
          queries_count = :queries_count,
          checkout_time_irmin = :checkout_time_irmin,
          checkout_time_tezedge = :checkout_time_tezedge,
          commit_time_irmin = :commit_time_irmin,
          commit_time_tezedge = :commit_time_tezedge
        WHERE
          id = :block_id;
            ",
        )?;

        query.execute(named_params! {
            ":queries_count": &self.nqueries,
            ":checkout_time_irmin": &self.checkout_time.as_ref().map(|(irmin, _)| irmin),
            ":checkout_time_tezedge": &self.checkout_time.as_ref().map(|(_, tezedge)| tezedge),
            ":commit_time_irmin": &commit_time_irmin,
            ":commit_time_tezedge": &commit_time_tezedge,
            ":block_id": block_id
        })?;

        for query in self.tezedge_global_stats.values_mut() {
            query.compute_mean();
        }
        for query in self.irmin_global_stats.values_mut() {
            query.compute_mean();
        }
        self.tezedge_checkout_stats.compute_mean();
        self.irmin_checkout_stats.compute_mean();
        self.tezedge_commit_stats.compute_mean();
        self.irmin_commit_stats.compute_mean();

        for (global_stats, commits, checkouts, name) in &mut [
            (
                &self.tezedge_global_stats,
                &self.tezedge_commit_stats,
                &self.tezedge_checkout_stats,
                "tezedge",
            ),
            (
                &self.irmin_global_stats,
                &self.irmin_commit_stats,
                &self.irmin_checkout_stats,
                "irmin",
            ),
        ] {
            for (root, query_stats) in global_stats.iter() {
                let root = root.as_str();

                self.insert_query_stats(sql, name, root, "mem", &query_stats.mem)?;
                self.insert_query_stats(sql, name, root, "mem_tree", &query_stats.mem_tree)?;
                self.insert_query_stats(sql, name, root, "find", &query_stats.find)?;
                self.insert_query_stats(sql, name, root, "find_tree", &query_stats.find_tree)?;
                self.insert_query_stats(sql, name, root, "add", &query_stats.add)?;
                self.insert_query_stats(sql, name, root, "add_tree", &query_stats.add_tree)?;
                self.insert_query_stats(sql, name, root, "remove", &query_stats.remove)?;
            }

            self.insert_query_stats(sql, name, "commit", "commit", commits)?;
            self.insert_query_stats(sql, name, "checkout", "checkout", checkouts)?;
        }

        Ok(())
    }

    fn insert_query_stats(
        &self,
        sql: &Connection,
        context_name: &str,
        root: &str,
        query_name: &str,
        range_stats: &RangeStats,
    ) -> Result<(), SQLError> {
        let mut query = sql.prepare_cached(
            "
        INSERT OR IGNORE INTO global_query_stats
          (root, query_name, context_name)
        VALUES
          (:root, :query_name, :context_name)
            ",
        )?;

        query.execute(named_params! {
            ":root": root,
            ":query_name": query_name,
            ":context_name": context_name,
        })?;

        let mut query = sql.prepare_cached(
            "
        UPDATE
          global_query_stats
        SET
          total_time = :total_time,
          queries_count = :queries_count,
          one_to_ten_us_count = :one_to_ten_us_count,
          one_to_ten_us_mean_time = :one_to_ten_us_mean_time,
          one_to_ten_us_max_time = :one_to_ten_us_max_time,
          one_to_ten_us_total_time = :one_to_ten_us_total_time,
          ten_to_one_hundred_us_count = :ten_to_one_hundred_us_count,
          ten_to_one_hundred_us_mean_time = :ten_to_one_hundred_us_mean_time,
          ten_to_one_hundred_us_max_time = :ten_to_one_hundred_us_max_time,
          ten_to_one_hundred_us_total_time = :ten_to_one_hundred_us_total_time,
          one_hundred_us_to_one_ms_count = :one_hundred_us_to_one_ms_count,
          one_hundred_us_to_one_ms_mean_time = :one_hundred_us_to_one_ms_mean_time,
          one_hundred_us_to_one_ms_max_time = :one_hundred_us_to_one_ms_max_time,
          one_hundred_us_to_one_ms_total_time = :one_hundred_us_to_one_ms_total_time,
          one_to_ten_ms_count = :one_to_ten_ms_count,
          one_to_ten_ms_mean_time = :one_to_ten_ms_mean_time,
          one_to_ten_ms_max_time = :one_to_ten_ms_max_time,
          one_to_ten_ms_total_time = :one_to_ten_ms_total_time,
          ten_to_one_hundred_ms_count = :ten_to_one_hundred_ms_count,
          ten_to_one_hundred_ms_mean_time = :ten_to_one_hundred_ms_mean_time,
          ten_to_one_hundred_ms_max_time = :ten_to_one_hundred_ms_max_time,
          ten_to_one_hundred_ms_total_time = :ten_to_one_hundred_ms_total_time,
          one_hundred_ms_to_one_s_count = :one_hundred_ms_to_one_s_count,
          one_hundred_ms_to_one_s_mean_time = :one_hundred_ms_to_one_s_mean_time,
          one_hundred_ms_to_one_s_max_time = :one_hundred_ms_to_one_s_max_time,
          one_hundred_ms_to_one_s_total_time = :one_hundred_ms_to_one_s_total_time,
          one_to_ten_s_count = :one_to_ten_s_count,
          one_to_ten_s_mean_time = :one_to_ten_s_mean_time,
          one_to_ten_s_max_time = :one_to_ten_s_max_time,
          one_to_ten_s_total_time = :one_to_ten_s_total_time,
          ten_to_one_hundred_s_count = :ten_to_one_hundred_s_count,
          ten_to_one_hundred_s_mean_time = :ten_to_one_hundred_s_mean_time,
          ten_to_one_hundred_s_max_time = :ten_to_one_hundred_s_max_time,
          ten_to_one_hundred_s_total_time = :ten_to_one_hundred_s_total_time,
          one_hundred_s_count = :one_hundred_s_count,
          one_hundred_s_mean_time = :one_hundred_s_mean_time,
          one_hundred_s_max_time = :one_hundred_s_max_time,
          one_hundred_s_total_time = :one_hundred_s_total_time
        WHERE
          root = :root AND query_name = :query_name AND context_name = :context_name;
        ",
        )?;

        query.execute(
            named_params! {
                ":root": root,
                ":query_name": query_name,
                ":context_name": context_name,
                ":total_time": &range_stats.total_time,
                ":queries_count": &range_stats.queries_count,
                ":one_to_ten_us_count": &range_stats.one_to_ten_us.count,
                ":one_to_ten_us_mean_time": &range_stats.one_to_ten_us.mean_time,
                ":one_to_ten_us_max_time": &range_stats.one_to_ten_us.max_time,
                ":one_to_ten_us_total_time": &range_stats.one_to_ten_us.total_time,
                ":ten_to_one_hundred_us_count": &range_stats.ten_to_one_hundred_us.count,
                ":ten_to_one_hundred_us_mean_time": &range_stats.ten_to_one_hundred_us.mean_time,
                ":ten_to_one_hundred_us_max_time": &range_stats.ten_to_one_hundred_us.max_time,
                ":ten_to_one_hundred_us_total_time": &range_stats.ten_to_one_hundred_us.total_time,
                ":one_hundred_us_to_one_ms_count": &range_stats.one_hundred_us_to_one_ms.count,
                ":one_hundred_us_to_one_ms_mean_time": &range_stats.one_hundred_us_to_one_ms.mean_time,
                ":one_hundred_us_to_one_ms_max_time": &range_stats.one_hundred_us_to_one_ms.max_time,
                ":one_hundred_us_to_one_ms_total_time": &range_stats.one_hundred_us_to_one_ms.total_time,
                ":one_to_ten_ms_count": &range_stats.one_to_ten_ms.count,
                ":one_to_ten_ms_mean_time": &range_stats.one_to_ten_ms.mean_time,
                ":one_to_ten_ms_max_time": &range_stats.one_to_ten_ms.max_time,
                ":one_to_ten_ms_total_time": &range_stats.one_to_ten_ms.total_time,
                ":ten_to_one_hundred_ms_count": &range_stats.ten_to_one_hundred_ms.count,
                ":ten_to_one_hundred_ms_mean_time": &range_stats.ten_to_one_hundred_ms.mean_time,
                ":ten_to_one_hundred_ms_max_time": &range_stats.ten_to_one_hundred_ms.max_time,
                ":ten_to_one_hundred_ms_total_time": &range_stats.ten_to_one_hundred_ms.total_time,
                ":one_hundred_ms_to_one_s_count": &range_stats.one_hundred_ms_to_one_s.count,
                ":one_hundred_ms_to_one_s_mean_time": &range_stats.one_hundred_ms_to_one_s.mean_time,
                ":one_hundred_ms_to_one_s_max_time": &range_stats.one_hundred_ms_to_one_s.max_time,
                ":one_hundred_ms_to_one_s_total_time": &range_stats.one_hundred_ms_to_one_s.total_time,
                ":one_to_ten_s_count": &range_stats.one_to_ten_s.count,
                ":one_to_ten_s_mean_time": &range_stats.one_to_ten_s.mean_time,
                ":one_to_ten_s_max_time": &range_stats.one_to_ten_s.max_time,
                ":one_to_ten_s_total_time": &range_stats.one_to_ten_s.total_time,
                ":ten_to_one_hundred_s_count": &range_stats.ten_to_one_hundred_s.count,
                ":ten_to_one_hundred_s_mean_time": &range_stats.ten_to_one_hundred_s.mean_time,
                ":ten_to_one_hundred_s_max_time": &range_stats.ten_to_one_hundred_s.max_time,
                ":ten_to_one_hundred_s_total_time": &range_stats.ten_to_one_hundred_s.total_time,
                ":one_hundred_s_count": &range_stats.one_hundred_s.count,
                ":one_hundred_s_mean_time": &range_stats.one_hundred_s.mean_time,
                ":one_hundred_s_max_time": &range_stats.one_hundred_s.max_time,
                ":one_hundred_s_total_time": &range_stats.one_hundred_s.total_time,
            },
        )?;

        Ok(())
    }

    fn init_sqlite(db_path: Option<PathBuf>) -> Result<Connection, SQLError> {
        let connection = match db_path {
            Some(mut path) => {
                if !path.is_dir() {
                    std::fs::create_dir_all(&path).ok();
                }

                path.push(FILENAME_DB);

                std::fs::remove_file(&path).ok();
                Connection::open(&path)?
            }
            None => Connection::open_in_memory()?,
        };

        let schema = include_str!("schema_stats.sql");

        let mut batch = Batch::new(&connection, schema);
        while let Some(mut stmt) = batch.next()? {
            stmt.execute([])?;
        }

        Ok(connection)
    }
}

#[cfg(test)]
mod tests {
    use crypto::hash::HashTrait;

    use super::*;

    #[test]
    fn test_timing_db() {
        let sql = Timing::init_sqlite(None).unwrap();
        let mut timing = Timing::new();
        let mut transaction = None;

        assert!(timing.current_block.is_none());

        let block_hash = BlockHash::try_from_bytes(&[1; 32]).unwrap();
        timing
            .set_current_block(
                &sql,
                Some(block_hash.clone()),
                None,
                Instant::now(),
                &mut transaction,
            )
            .unwrap();
        let block_id = timing.current_block.clone().unwrap().0;

        timing
            .set_current_block(
                &sql,
                Some(block_hash),
                None,
                Instant::now(),
                &mut transaction,
            )
            .unwrap();
        let same_block_id = timing.current_block.clone().unwrap().0;

        assert_eq!(block_id, same_block_id);

        timing
            .set_current_block(
                &sql,
                Some(BlockHash::try_from_bytes(&[2; 32]).unwrap()),
                None,
                Instant::now(),
                &mut transaction,
            )
            .unwrap();
        let other_block_id = timing.current_block.clone().unwrap().0;

        assert_ne!(block_id, other_block_id);

        timing
            .insert_query(
                &sql,
                &mut transaction,
                &Query {
                    query_name: QueryKind::Mem,
                    key: vec!["a", "b", "c"]
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                    irmin_time: Some(1.0),
                    tezedge_time: Some(2.0),
                },
            )
            .unwrap();

        timing.sync_block_stats(&sql).unwrap();
        timing
            .sync_global_stats(&sql, Some(1.0), Some(1.0))
            .unwrap();
    }

    #[test]
    fn test_queries_db() {
        let block_hash = BlockHash::try_from_bytes(&[1; 32]).unwrap();
        let context_hash = ContextHash::try_from_bytes(&[2; 32]).unwrap();

        TIMING_CHANNEL
            .send(TimingMessage::InitTiming { db_path: None })
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::SetBlock {
                block_hash: Some(block_hash),
                timestamp: None,
                instant: Instant::now(),
            })
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Checkout {
                context_hash,
                irmin_time: Some(1.0),
                tezedge_time: Some(2.0),
            })
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Query(Query {
                query_name: QueryKind::Add,
                key: vec!["a", "b", "c"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(1.0),
                tezedge_time: Some(2.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Query(Query {
                query_name: QueryKind::Find,
                key: vec!["a", "b", "c"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(5.0),
                tezedge_time: Some(6.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Query(Query {
                query_name: QueryKind::Find,
                key: vec!["a", "b", "c"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(50.0),
                tezedge_time: Some(60.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Query(Query {
                query_name: QueryKind::Mem,
                key: vec!["m", "n", "o"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(10.0),
                tezedge_time: Some(20.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Query(Query {
                query_name: QueryKind::Add,
                key: vec!["m", "n", "o"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(15.0),
                tezedge_time: Some(26.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Query(Query {
                query_name: QueryKind::Add,
                key: vec!["m", "n", "o"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(150.0),
                tezedge_time: Some(260.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Commit {
                irmin_time: Some(15.0),
                tezedge_time: Some(20.0),
            })
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
