// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cell::RefCell;
use std::{
    collections::HashMap,
    convert::TryInto,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};

use container::{
    InlinedBlockHash, InlinedContextHash, InlinedOperationHash, InlinedProtocolHash, InlinedString,
};
use crypto::hash::ProtocolHash;
use rusqlite::{named_params, Batch, Connection, Error as SQLError, Transaction};
use serde::Serialize;
use static_assertions::assert_eq_size;
use tezos_spsc::{
    bounded, Consumer,
    PopError::{Closed, Empty},
    Producer, PushError,
};

pub mod container;

pub const FILENAME_DB: &str = "context_stats.db";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    Genesis,
    Bootstrap,
    Alpha1,
    Alpha2,
    Alpha3,
    AthensA,
    Babylon,
    Carthage,
    Delphi,
    Edo,
    Florence,
    Granada,
    Hangzhou,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Genesis => "genesis",
            Protocol::Bootstrap => "bootstrap",
            Protocol::Alpha1 => "alpha1",
            Protocol::Alpha2 => "alpha2",
            Protocol::Alpha3 => "alpha3",
            Protocol::AthensA => "athens_a",
            Protocol::Babylon => "babylon",
            Protocol::Carthage => "carthage",
            Protocol::Delphi => "delphi",
            Protocol::Edo => "edo",
            Protocol::Florence => "florence",
            Protocol::Granada => "granada",
            Protocol::Hangzhou => "hangzhou",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Protocol> {
        match s {
            "genesis" => Some(Protocol::Genesis),
            "bootstrap" => Some(Protocol::Bootstrap),
            "alpha1" => Some(Protocol::Alpha1),
            "alpha2" => Some(Protocol::Alpha2),
            "alpha3" => Some(Protocol::Alpha3),
            "athens_a" => Some(Protocol::AthensA),
            "babylon" => Some(Protocol::Babylon),
            "carthage" => Some(Protocol::Carthage),
            "delphi" => Some(Protocol::Delphi),
            "edo" => Some(Protocol::Edo),
            "florence" => Some(Protocol::Florence),
            "granada" => Some(Protocol::Granada),
            "hangzhou" => Some(Protocol::Hangzhou),
            _ => None,
        }
    }
}

pub const STATS_ALL_PROTOCOL: &str = "_all_";

const PROTO_HASH_GENESIS: &str = "PrihK96nBAFSxVL1GLJTVhu9YnzkMFiBeuJRPA8NwuZVZCE1L6i";
const PROTO_HASH_BOOTSTRAP: &str = "Ps9mPmXaRzmzk35gbAYNCAw6UXdE2qoABTHbN2oEEc1qM7CwT9P";
const PROTO_HASH_ALPHA1: &str = "PtCJ7pwoxe8JasnHY8YonnLYjcVHmhiARPJvqcC6VfHT5s8k8sY";
const PROTO_HASH_ALPHA2: &str = "PsYLVpVvgbLhAhoqAkMFUo6gudkJ9weNXhUYCiLDzcUpFpkk8Wt";
const PROTO_HASH_ALPHA3: &str = "PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP";
const PROTO_HASH_ATHENS_A: &str = "Pt24m4xiPbLDhVgVfABUjirbmda3yohdN82Sp9FeuAXJ4eV9otd";
const PROTO_HASH_BABYLON: &str = "PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS";
const PROTO_HASH_CARTHAGE: &str = "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb";
const PROTO_HASH_DELPHI: &str = "PsDELPH1Kxsxt8f9eWbxQeRxkjfbxoqM52jvs5Y5fBxWWh4ifpo";
const PROTO_HASH_EDO: &str = "PtEdo2ZkT9oKpimTah6x2embF25oss54njMuPzkJTEi5RqfdZFA";
const PROTO_HASH_FLORENCE: &str = "PsFLorenaUUuikDWvMDr6fGBRG8kt3e3D3fHoXK1j1BFRxeSH4i";
const PROTO_HASH_GRANADA: &str = "PtGRANADsDU8R9daYKAgWnQYAJ64omN1o3KMGVCykShA97vQbvV";
const PROTO_HASH_HANGZHOU: &str = "PtHangz2aRngywmSRGGvrcTyMbbdpWdpFKuS4uMWxg2RaH9i1qx";

const PROTOCOLS: &[(&str, Protocol)] = &[
    (PROTO_HASH_GENESIS, Protocol::Genesis),
    (PROTO_HASH_BOOTSTRAP, Protocol::Bootstrap),
    (PROTO_HASH_ALPHA1, Protocol::Alpha1),
    (PROTO_HASH_ALPHA2, Protocol::Alpha2),
    (PROTO_HASH_ALPHA3, Protocol::Alpha3),
    (PROTO_HASH_ATHENS_A, Protocol::AthensA),
    (PROTO_HASH_BABYLON, Protocol::Babylon),
    (PROTO_HASH_CARTHAGE, Protocol::Carthage),
    (PROTO_HASH_DELPHI, Protocol::Delphi),
    (PROTO_HASH_EDO, Protocol::Edo),
    (PROTO_HASH_FLORENCE, Protocol::Florence),
    (PROTO_HASH_GRANADA, Protocol::Granada),
    (PROTO_HASH_HANGZHOU, Protocol::Hangzhou),
];

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
    pub highest_hash_id: u64,
    pub ndirectories: usize,
    pub nblobs: usize,
    pub nblobs_inlined: usize,
    pub nshapes: usize,
    pub ninode_pointers: usize,
    pub offset_length: usize,
    pub total_bytes: usize,
}

impl SerializeStats {
    pub fn add_directory(
        &mut self,
        keys_length: usize,
        nblobs_inlined: usize,
        blobs_length: usize,
    ) {
        self.ndirectories += 1;
        self.keys_length += keys_length;
        self.nblobs_inlined += nblobs_inlined;
        self.blobs_length += blobs_length;
    }

    pub fn add_shape(&mut self, nblobs_inlined: usize, blobs_length: usize) {
        self.nshapes += 1;
        self.nblobs_inlined += nblobs_inlined;
        self.blobs_length += blobs_length;
    }

    pub fn add_inode_pointers(&mut self) {
        self.ninode_pointers += 1;
    }

    pub fn add_offset(&mut self, offset_length: usize) {
        self.offset_length += offset_length;
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
    pub temp_inodes_index: usize,
    pub blobs_len: usize,
    pub blobs_cap: usize,
    pub inodes_len: usize,
    pub inodes_cap: usize,
    pub fat_pointers_cap: usize,
    pub thin_pointers_cap: usize,
    pub pointers_data_len: usize,
    pub strings: StringsMemoryUsage,
    pub total_bytes: usize,
}

#[derive(Debug, Default)]
pub struct StringsMemoryUsage {
    pub all_strings_map_cap: usize,
    pub all_strings_map_len: usize,
    pub all_strings_to_serialize_cap: usize,
    pub all_strings_cap: usize,
    pub all_strings_len: usize,
    pub big_strings_cap: usize,
    pub big_strings_len: usize,
    pub big_strings_map_cap: usize,
    pub big_strings_map_len: usize,
    pub big_strings_hashes_bytes: usize,
    pub total_bytes: usize,
}

#[derive(Debug, Default)]
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
    /// Number of items in the queue of `HashId`
    pub npending_free_ids: usize,
    /// Number of items in the GC thread waiting to be pushed in the queue
    pub gc_npending_free_ids: usize,
    /// Number of shapes
    pub nshapes: usize,
    /// Bytes occupied by all strings in the repository
    pub strings_total_bytes: usize,
    /// Bytes occupied by shapes in the repository
    pub shapes_total_bytes: usize,
    /// Bytes occupied by commit index in the repository
    pub commit_index_total_bytes: usize,
    /// Capacity of `HashValueStore::new_ids`
    pub new_ids_cap: usize,
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
        block_hash: Option<InlinedBlockHash>,
        /// Duration since std::time::UNIX_EPOCH.
        /// It is `None` when `block_hash` is `None`.
        timestamp: Option<Duration>,
        /// Instant when the hook `set_block` was called.
        /// `Instant` is required because it's monotonic, `SystemTime` (which
        /// is used to get `timestamp`) is not.
        instant: Instant,
    },
    SetProtocol(InlinedProtocolHash),
    SetOperation(Option<InlinedOperationHash>),
    Checkout {
        context_hash: InlinedContextHash,
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

assert_eq_size!([u8; 576], TimingMessage);
assert_eq_size!([u8; 16], Option<f64>);

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

#[derive(Default)]
struct GlobalStatistics {
    tezedge_global_stats: HashMap<String, QueryStatsWithRange>,
    irmin_global_stats: HashMap<String, QueryStatsWithRange>,
    tezedge_commit_stats: RangeStats,
    irmin_commit_stats: RangeStats,
    tezedge_checkout_stats: RangeStats,
    irmin_checkout_stats: RangeStats,
}

impl GlobalStatistics {
    fn add_commit_times(&mut self, irmin_time: Option<f64>, tezedge_time: Option<f64>) {
        self.tezedge_commit_stats.add_time(tezedge_time);
        self.irmin_commit_stats.add_time(irmin_time);
    }

    fn add_checkout_times(&mut self, irmin_time: Option<f64>, tezedge_time: Option<f64>) {
        self.tezedge_checkout_stats.add_time(tezedge_time);
        self.irmin_checkout_stats.add_time(irmin_time);
    }

    fn compute_mean(&mut self) {
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
    }

    fn update_stats(&mut self, root: &str, query: &Query) {
        for (global_stats, time) in &mut [
            (Some(&mut self.tezedge_global_stats), query.tezedge_time),
            (Some(&mut self.irmin_global_stats), query.irmin_time),
        ] {
            let global_stats = match global_stats {
                Some(global_stats) => global_stats,
                None => continue,
            };

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
            let query_stats = match query.query_kind {
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
}

struct Timing {
    current_block: Option<(HashId, InlinedBlockHash)>,
    current_operation: Option<(HashId, InlinedOperationHash)>,
    current_context: Option<(HashId, InlinedContextHash)>,
    block_started_at: Option<(Duration, Instant)>,
    /// Number of queries in current block
    nqueries: usize,
    /// Checkout time for the current block
    checkout_time: Option<(Option<f64>, Option<f64>)>,
    /// Statistics for the current block
    block_stats: HashMap<String, QueryStats>,
    /// Global statistics
    global: GlobalStatistics,
    protocol_tables: HashMap<InlinedProtocolHash, Protocol>,
    current_protocol: Option<Protocol>,
    stats_per_protocol: HashMap<Protocol, GlobalStatistics>,
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
    pub query_kind: QueryKind,
    pub key: InlinedString,
    pub irmin_time: Option<f64>,
    pub tezedge_time: Option<f64>,
}

#[derive(Default)]
pub struct TimingChannelShared {
    mutex: Mutex<()>,
    condvar: Condvar,
    is_waiting: AtomicBool,
}

pub struct TimingChannel {
    shared: Arc<TimingChannelShared>,
    producer: Producer<TimingMessage>,
}

#[derive(Debug)]
pub enum ChannelError {
    PtrNull,
    PushError(Box<PushError<TimingMessage>>),
}

impl TimingChannel {
    fn new(producer: Producer<TimingMessage>) -> Self {
        Self {
            shared: Arc::new(TimingChannelShared {
                mutex: Default::default(),
                condvar: Default::default(),
                is_waiting: AtomicBool::new(false),
            }),
            producer,
        }
    }

    pub fn send(&mut self, msg: TimingMessage) -> Result<(), ChannelError> {
        let result = self.producer.push(msg);

        if self.shared.is_waiting.load(Ordering::Acquire) {
            // Wake up the timing thread
            self.shared.condvar.notify_one();
        }

        result.map_err(|err| ChannelError::PushError(Box::new(err)))
    }
}

fn make_protocol_table() -> HashMap<InlinedProtocolHash, Protocol> {
    let mut table = HashMap::default();

    for (hash, protocol) in PROTOCOLS {
        let protocol_hash = ProtocolHash::from_base58_check(hash).unwrap();
        table.insert(
            InlinedProtocolHash::from(protocol_hash.0.as_slice()),
            *protocol,
        );
    }

    table
}

thread_local! {
    /// We put TIMING_CHANNEL in a thread local variable so that
    /// we can access `TimingChannel` mutably.
    /// `Producer::push` requires mutability.
    pub static TIMING_CHANNEL: RefCell<TimingChannel> = init_timing();
}

static TIMING_INITIALIZED: AtomicBool = AtomicBool::new(false);

fn init_timing() -> RefCell<TimingChannel> {
    let (producer, consumer) = bounded(20_000);

    let channel = TimingChannel::new(producer);

    // `init_timing` is called once per thread.
    // We want the timing thread to be started only once (per 1 thread).
    // If `init_timing` is called a second time by another thread, this is an error.
    if TIMING_INITIALIZED.swap(true, Ordering::SeqCst) {
        eprintln!("Error: Timing thread initialized more than once");
        return RefCell::new(channel);
    }

    let common = channel.shared.clone();

    if let Err(e) = std::thread::Builder::new()
        .name("ctx-timings-thread".to_string())
        .spawn(|| {
            start_timing(consumer, common);
        })
    {
        eprintln!("Fail to create timing channel: {:?}", e);
    }

    RefCell::new(channel)
}

fn start_timing(mut consumer: Consumer<TimingMessage>, shared: Arc<TimingChannelShared>) {
    let mut db_path: Option<PathBuf> = None;

    while let Ok(msg) = consumer.pop() {
        if let TimingMessage::InitTiming { db_path: path } = msg {
            db_path = path;
            break;
        };
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

    if let Err(e) = timing.reload_global_stats(&sql) {
        eprintln!("Fail to reload timing database {:?}", e);
    }

    let mut ntimes_empty = 0;

    loop {
        match consumer.pop() {
            Ok(msg) => {
                ntimes_empty = 0;
                if let Err(err) = timing.process_msg(&sql, &mut transaction, msg) {
                    eprintln!("Timing error={:?}", err);
                }
            }
            Err(Empty) => {
                if ntimes_empty < 5 {
                    // Sleep between 10 and 50 ms
                    std::thread::sleep(Duration::from_millis(10 * (ntimes_empty + 1)));
                    ntimes_empty += 1;
                    continue;
                }
                ntimes_empty = 0;

                // Let the main thread knows that we are waiting
                // Only this thread (timing) writes on `TimingChannelShared::is_waiting`
                // so no data race is possible: see below.
                shared.is_waiting.store(true, Ordering::Release);

                // If the main thread reads `TimingChannelShared::is_waiting` as `true`
                // at this point (before we called `CondVar::wait`), it won't
                // cause any harm because the main thread would call `notify_one` without
                // notifying anyone, but `is_waiting` will remains `true`.
                // So when the main thread reads a second time `is_waiting` (in another call),
                // it will notify again, and this time we will already have reached
                // the `CondVar::wait`.

                let guard = shared.mutex.lock().unwrap();
                let _guard = shared.condvar.wait(guard).unwrap();

                shared.is_waiting.store(false, Ordering::Release);
            }
            Err(Closed) => {
                return;
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
            protocol_tables: make_protocol_table(),
            current_protocol: None,
            global: Default::default(),
            stats_per_protocol: Default::default(),
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
            TimingMessage::SetProtocol(protocol_hash) => {
                self.set_current_protocol(&protocol_hash);
                Ok(())
            }
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

    fn insert_block_memory_usage(
        &mut self,
        sql: &Connection,
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
              repo_npending_free_ids = :repo_npending_free_ids,
              repo_gc_npending_free_ids = :repo_gc_npending_free_ids,
              repo_nshapes = :repo_nshapes,
              repo_strings_total_bytes = :repo_strings_total_bytes,
              repo_shapes_total_bytes = :repo_shapes_total_bytes,
              repo_commit_index_total_bytes = :repo_commit_index_total_bytes,
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
              serialize_nshapes = :serialize_nshapes,
              serialize_ninode_pointers = :serialize_ninode_pointers,
              serialize_offset_length = :serialize_offset_length,
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
            ":repo_npending_free_ids": stats.context.repo.npending_free_ids,
            ":repo_gc_npending_free_ids": stats.context.repo.gc_npending_free_ids,
            ":repo_nshapes": stats.context.repo.nshapes,
            ":repo_strings_total_bytes": stats.context.repo.strings_total_bytes,
            ":repo_shapes_total_bytes": stats.context.repo.shapes_total_bytes,
            ":repo_commit_index_total_bytes": stats.context.repo.commit_index_total_bytes,
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
            ":serialize_nshapes": stats.serialize.nshapes,
            ":serialize_ninode_pointers": stats.serialize.ninode_pointers,
            ":serialize_offset_length": stats.serialize.offset_length,
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
        block_hash: Option<InlinedBlockHash>,
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

    fn set_current_protocol(&mut self, protocol_hash: &InlinedProtocolHash) {
        let protocol = match self.protocol_tables.get(protocol_hash).copied() {
            Some(protocol) => protocol,
            None => return,
        };

        if Some(protocol) != self.current_protocol {
            self.current_protocol.replace(protocol);
        }
    }

    fn set_current_operation(
        &mut self,
        sql: &Connection,
        operation_hash: Option<InlinedOperationHash>,
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
        context_hash: InlinedContextHash,
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
        T: AsRef<[u8]>,
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
        context_hash: InlinedContextHash,
        irmin_time: Option<f64>,
        tezedge_time: Option<f64>,
    ) -> Result<(), SQLError> {
        if self.current_block.is_none() {
            return Ok(());
        }

        self.global.add_checkout_times(irmin_time, tezedge_time);

        if let Some(protocol_stats) = self.get_stats_protocol_mut() {
            protocol_stats.add_checkout_times(irmin_time, tezedge_time);
        };

        self.set_current_context(sql, context_hash)?;
        self.checkout_time = Some((irmin_time, tezedge_time));

        Ok(())
    }

    fn get_stats_protocol_mut(&mut self) -> Option<&mut GlobalStatistics> {
        let protocol = match self.current_protocol {
            Some(protocol) => protocol,
            None => return None,
        };

        Some(self.stats_per_protocol.entry(protocol).or_default())
    }

    fn get_stats_protocol(&self) -> Option<(Protocol, &GlobalStatistics)> {
        let protocol = match self.current_protocol {
            Some(protocol) => protocol,
            None => return None,
        };

        Some((protocol, self.stats_per_protocol.get(&protocol)?))
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

        self.global.add_commit_times(irmin_time, tezedge_time);

        if let Some(protocol_stats) = self.get_stats_protocol_mut() {
            protocol_stats.add_commit_times(irmin_time, tezedge_time);
        };

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
        let query_name = query.query_kind.to_str();

        let root = match query.key.as_str().split_once('/') {
            Some((root, _)) if !root.is_empty() => Some(root),
            _ => None,
        };

        // TODO - TE-261: disabled for now because it is not used for anything
        // Re-enable once needed, and maybe add a command-line flag for it.
        // We probably want to also add some kind of garbage collection to only keep
        // values for the last N cycles, and not everything since the beginning.
        if false {
            let key = query.key.as_str();

            let mut stmt = sql.prepare_cached("INSERT OR IGNORE INTO keys (key) VALUES (?1)")?;
            stmt.execute([key])?;

            let mut stmt = sql.prepare_cached("SELECT id FROM keys WHERE key = ?1;")?;

            let key_id: usize = stmt.query_row([key], |row| row.get(0))?;

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

        self.add_block_stats(root, query);
        self.add_global_stats(root, query);

        Ok(())
    }

    fn add_global_stats(&mut self, root: &str, query: &Query) {
        self.global.update_stats(root, query);

        if let Some(protocol_stats) = self.get_stats_protocol_mut() {
            protocol_stats.update_stats(root, query);
        };
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

        let (value_tezedge, value_irmin) = match query.query_kind {
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

        self.global.compute_mean();

        if let Some(protocol_stats) = self.get_stats_protocol_mut() {
            protocol_stats.compute_mean();
        };

        self.sync_global_stats_impl(sql, &self.global, None)?;

        if let Some((protocol, stats)) = self.get_stats_protocol() {
            self.sync_global_stats_impl(sql, stats, Some(protocol))?;
        }

        Ok(())
    }

    fn sync_global_stats_impl(
        &self,
        sql: &Connection,
        global: &GlobalStatistics,
        protocol: Option<Protocol>,
    ) -> Result<(), SQLError> {
        for (global_stats, commits, checkouts, name) in &mut [
            (
                &global.tezedge_global_stats,
                &global.tezedge_commit_stats,
                &global.tezedge_checkout_stats,
                "tezedge",
            ),
            (
                &global.irmin_global_stats,
                &global.irmin_commit_stats,
                &global.irmin_checkout_stats,
                "irmin",
            ),
        ] {
            for (root, query) in global_stats.iter() {
                let root = root.as_str();

                self.insert_query_stats(sql, name, root, "mem", &query.mem, protocol)?;
                self.insert_query_stats(sql, name, root, "mem_tree", &query.mem_tree, protocol)?;
                self.insert_query_stats(sql, name, root, "find", &query.find, protocol)?;
                self.insert_query_stats(sql, name, root, "find_tree", &query.find_tree, protocol)?;
                self.insert_query_stats(sql, name, root, "add", &query.add, protocol)?;
                self.insert_query_stats(sql, name, root, "add_tree", &query.add_tree, protocol)?;
                self.insert_query_stats(sql, name, root, "remove", &query.remove, protocol)?;
            }

            self.insert_query_stats(sql, name, "commit", "commit", commits, protocol)?;
            self.insert_query_stats(sql, name, "checkout", "checkout", checkouts, protocol)?;
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
        protocol: Option<Protocol>,
    ) -> Result<(), SQLError> {
        let mut query = sql.prepare_cached(
            "
        INSERT OR IGNORE INTO global_query_stats
          (root, query_name, context_name, protocol)
        VALUES
          (:root, :query_name, :context_name, :protocol)
            ",
        )?;

        let protocol = protocol
            .as_ref()
            .map(|p| p.as_str())
            .unwrap_or(STATS_ALL_PROTOCOL);

        query.execute(named_params! {
            ":root": root,
            ":query_name": query_name,
            ":context_name": context_name,
            ":protocol": protocol,
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
          root = :root AND query_name = :query_name AND context_name = :context_name AND protocol = :protocol;
        ",
        )?;

        query.execute(
            named_params! {
                ":root": root,
                ":query_name": query_name,
                ":context_name": context_name,
                ":total_time": &range_stats.total_time,
                ":queries_count": &range_stats.queries_count,
                ":protocol": &protocol,
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

    fn reload_global_stats(&mut self, sql: &Connection) -> Result<(), SQLError> {
        let mut stmt = sql.prepare(
            "
    SELECT
      query_name,
      root,
      one_to_ten_us_count,
      one_to_ten_us_mean_time,
      one_to_ten_us_max_time,
      one_to_ten_us_total_time,
      ten_to_one_hundred_us_count,
      ten_to_one_hundred_us_mean_time,
      ten_to_one_hundred_us_max_time,
      ten_to_one_hundred_us_total_time,
      one_hundred_us_to_one_ms_count,
      one_hundred_us_to_one_ms_mean_time,
      one_hundred_us_to_one_ms_max_time,
      one_hundred_us_to_one_ms_total_time,
      one_to_ten_ms_count,
      one_to_ten_ms_mean_time,
      one_to_ten_ms_max_time,
      one_to_ten_ms_total_time,
      ten_to_one_hundred_ms_count,
      ten_to_one_hundred_ms_mean_time,
      ten_to_one_hundred_ms_max_time,
      ten_to_one_hundred_ms_total_time,
      one_hundred_ms_to_one_s_count,
      one_hundred_ms_to_one_s_mean_time,
      one_hundred_ms_to_one_s_max_time,
      one_hundred_ms_to_one_s_total_time,
      one_to_ten_s_count,
      one_to_ten_s_mean_time,
      one_to_ten_s_max_time,
      one_to_ten_s_total_time,
      ten_to_one_hundred_s_count,
      ten_to_one_hundred_s_mean_time,
      ten_to_one_hundred_s_max_time,
      ten_to_one_hundred_s_total_time,
      one_hundred_s_count,
      one_hundred_s_mean_time,
      one_hundred_s_max_time,
      one_hundred_s_total_time,
      total_time,
      queries_count,
      context_name,
      protocol
    FROM
      global_query_stats
       ",
        )?;

        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            let context_name: &str = match row.get_ref(40)?.as_str() {
                Ok(context_name) => context_name,
                Err(_) => continue,
            };

            let protocol = row.get_ref(41)?.as_str().ok().and_then(Protocol::from_str);

            let global_stats = match protocol {
                Some(protocol) => self.stats_per_protocol.entry(protocol).or_default(),
                None => &mut self.global,
            };

            let (global_stats, commit_stats, checkout_stats) = match context_name {
                "tezedge" => (
                    &mut global_stats.tezedge_global_stats,
                    &mut global_stats.tezedge_commit_stats,
                    &mut global_stats.tezedge_checkout_stats,
                ),
                "irmin" => (
                    &mut global_stats.irmin_global_stats,
                    &mut global_stats.irmin_commit_stats,
                    &mut global_stats.irmin_checkout_stats,
                ),
                _ => continue,
            };

            let query_name = match row.get_ref(0)?.as_str() {
                Ok(name) if !name.is_empty() => name,
                _ => continue,
            };

            let root = match row.get_ref(1)?.as_str() {
                Ok(root) if !root.is_empty() => root,
                _ => continue,
            };

            let mut query_stats = match query_name {
                "commit" => commit_stats,
                "checkout" => checkout_stats,
                _ => {
                    let entry = match global_stats.get_mut(root) {
                        Some(entry) => entry,
                        None => {
                            let stats = QueryStatsWithRange {
                                root: root.to_string(),
                                ..Default::default()
                            };
                            global_stats.insert(root.to_string(), stats);
                            global_stats.get_mut(root).unwrap()
                        }
                    };
                    match query_name {
                        "mem" => &mut entry.mem,
                        "mem_tree" => &mut entry.mem_tree,
                        "find" => &mut entry.find,
                        "find_tree" => &mut entry.find_tree,
                        "add" => &mut entry.add,
                        "add_tree" => &mut entry.add_tree,
                        "remove" => &mut entry.remove,
                        _ => continue,
                    }
                }
            };

            query_stats.one_to_ten_us.count = row.get(2)?;
            query_stats.one_to_ten_us.mean_time = row.get(3)?;
            query_stats.one_to_ten_us.max_time = row.get(4)?;
            query_stats.one_to_ten_us.total_time = row.get(5)?;
            query_stats.ten_to_one_hundred_us.count = row.get(6)?;
            query_stats.ten_to_one_hundred_us.mean_time = row.get(7)?;
            query_stats.ten_to_one_hundred_us.max_time = row.get(8)?;
            query_stats.ten_to_one_hundred_us.total_time = row.get(9)?;
            query_stats.one_hundred_us_to_one_ms.count = row.get(10)?;
            query_stats.one_hundred_us_to_one_ms.mean_time = row.get(11)?;
            query_stats.one_hundred_us_to_one_ms.max_time = row.get(12)?;
            query_stats.one_hundred_us_to_one_ms.total_time = row.get(13)?;
            query_stats.one_to_ten_ms.count = row.get(14)?;
            query_stats.one_to_ten_ms.mean_time = row.get(15)?;
            query_stats.one_to_ten_ms.max_time = row.get(16)?;
            query_stats.one_to_ten_ms.total_time = row.get(17)?;
            query_stats.ten_to_one_hundred_ms.count = row.get(18)?;
            query_stats.ten_to_one_hundred_ms.mean_time = row.get(19)?;
            query_stats.ten_to_one_hundred_ms.max_time = row.get(20)?;
            query_stats.ten_to_one_hundred_ms.total_time = row.get(21)?;
            query_stats.one_hundred_ms_to_one_s.count = row.get(22)?;
            query_stats.one_hundred_ms_to_one_s.mean_time = row.get(23)?;
            query_stats.one_hundred_ms_to_one_s.max_time = row.get(24)?;
            query_stats.one_hundred_ms_to_one_s.total_time = row.get(25)?;
            query_stats.one_to_ten_s.count = row.get(26)?;
            query_stats.one_to_ten_s.mean_time = row.get(27)?;
            query_stats.one_to_ten_s.max_time = row.get(28)?;
            query_stats.one_to_ten_s.total_time = row.get(29)?;
            query_stats.ten_to_one_hundred_s.count = row.get(30)?;
            query_stats.ten_to_one_hundred_s.mean_time = row.get(31)?;
            query_stats.ten_to_one_hundred_s.max_time = row.get(32)?;
            query_stats.ten_to_one_hundred_s.total_time = row.get(33)?;
            query_stats.one_hundred_s.count = row.get(34)?;
            query_stats.one_hundred_s.mean_time = row.get(35)?;
            query_stats.one_hundred_s.max_time = row.get(36)?;
            query_stats.one_hundred_s.total_time = row.get(37)?;
            query_stats.total_time = row.get(38)?;
            query_stats.queries_count = row.get(39)?;
        }

        Ok(())
    }

    fn init_sqlite(db_path: Option<PathBuf>) -> Result<Connection, SQLError> {
        let connection = match db_path {
            Some(mut path) => {
                if !path.is_dir() {
                    std::fs::create_dir_all(&path).ok();
                }

                path.push(FILENAME_DB);

                // std::fs::remove_file(&path).ok();
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
    use crate::container::{InlinedBlockHash, InlinedContextHash};

    use super::*;

    fn send_msg(msg: TimingMessage) -> Result<(), ChannelError> {
        TIMING_CHANNEL.with(|channel| {
            let mut channel = channel.borrow_mut();
            channel.send(msg)
        })
    }

    #[test]
    fn test_timing_db() {
        let sql = Timing::init_sqlite(None).unwrap();
        let mut timing = Timing::new();
        let mut transaction = None;

        assert!(timing.current_block.is_none());

        let block_hash = InlinedBlockHash::from(&[1; 32][..]);
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
                Some(InlinedBlockHash::from(&[2; 32][..])),
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
                    query_kind: QueryKind::Mem,
                    key: InlinedString::from(&["a", "b", "c"][..]),
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
    fn test_actions_db() {
        let block_hash = InlinedBlockHash::from(&[1; 32][..]);
        let context_hash = InlinedContextHash::from(&[2; 32][..]);

        send_msg(TimingMessage::InitTiming { db_path: None }).unwrap();
        send_msg(TimingMessage::SetBlock {
            block_hash: Some(block_hash),
            timestamp: None,
            instant: Instant::now(),
        })
        .unwrap();
        send_msg(TimingMessage::Checkout {
            context_hash,
            irmin_time: Some(1.0),
            tezedge_time: Some(2.0),
        })
        .unwrap();
        send_msg(TimingMessage::Query(Query {
            query_kind: QueryKind::Add,
            key: InlinedString::from(&["a", "b", "c"][..]),
            irmin_time: Some(1.0),
            tezedge_time: Some(2.0),
        }))
        .unwrap();
        send_msg(TimingMessage::Query(Query {
            query_kind: QueryKind::Find,
            key: InlinedString::from(&["a", "b", "c"][..]),
            irmin_time: Some(5.0),
            tezedge_time: Some(6.0),
        }))
        .unwrap();
        send_msg(TimingMessage::Query(Query {
            query_kind: QueryKind::Find,
            key: InlinedString::from(&["a", "b", "c"][..]),
            irmin_time: Some(50.0),
            tezedge_time: Some(60.0),
        }))
        .unwrap();
        send_msg(TimingMessage::Query(Query {
            query_kind: QueryKind::Mem,
            key: InlinedString::from(&["m", "n", "o"][..]),
            irmin_time: Some(10.0),
            tezedge_time: Some(20.0),
        }))
        .unwrap();
        send_msg(TimingMessage::Query(Query {
            query_kind: QueryKind::Add,
            key: InlinedString::from(&["m", "n", "o"][..]),
            irmin_time: Some(15.0),
            tezedge_time: Some(26.0),
        }))
        .unwrap();
        send_msg(TimingMessage::Query(Query {
            query_kind: QueryKind::Add,
            key: InlinedString::from(&["m", "n", "o"][..]),
            irmin_time: Some(150.0),
            tezedge_time: Some(260.0),
        }))
        .unwrap();
        send_msg(TimingMessage::Commit {
            irmin_time: Some(15.0),
            tezedge_time: Some(20.0),
        })
        .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
