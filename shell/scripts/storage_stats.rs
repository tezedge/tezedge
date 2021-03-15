use std::{fs, path::PathBuf, sync::Arc};

use storage::backend::{
    BTreeMapBackend, InMemoryBackend, MarkMoveGCed, MarkSweepGCed, RocksDBBackend, SledBackend,
};

use clap::{App, Arg};
use failure::Error;
use rocksdb::Cache;
use shell::context_listener::get_new_tree_hash;
use shell::context_listener::perform_context_action;
use slog::{debug, info, warn, Drain, Level, Logger};
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::sync::RwLock;
use storage::action_file::ActionsFileReader;
use storage::context::{ContextApi, TezedgeContext};
use storage::merkle_storage::MerkleStorage;
use storage::merkle_storage_stats::{MerkleStorageAction, OperationLatencyStats};
use storage::persistent::KeyValueSchema;

use tezos_context::channel::ContextAction;

struct Args {
    blocks_per_cycle: usize,
    blocks_limit: Option<usize>,
    input: String,
    output: String,
    backend: String,
}

impl Args {
    pub fn read_args() -> Self {
        let app = App::new("storage-stats")
            .about("Generate statistics for storage")
            .arg(Arg::with_name("input")
                 .long("input")
                 .takes_value(true)
                 .required(true)
                 .help("path to the actions.bin"))
            .arg(Arg::with_name("cycle-size")
                 .long("cycle_size")
                 .takes_value(true)
                 .required(true)
                 .default_value("2048")
                 .help("number of blocks in cycle"))
            .arg(Arg::with_name("blocks_limit")
                 .takes_value(true)
                 .long("blocks_limit")
                 .help("limits number of processed blocks"))
            .arg(Arg::with_name("output")
                 .takes_value(true)
                 .long("output")
                 .default_value("/tmp/storage-stats")
                 .help("output path for statistics"))
            .arg(Arg::with_name("backend")
                 .takes_value(true)
                 .long("backend")
                 .required(true)
                 .default_value("in-memory-gced")
                 .help("backend to use for storing merkle storage. Possible values: 'inmem', 'rocksdb', 'sled', 'mark_sweep', 'mark-move'"));

        let matches = app.get_matches();

        Self {
            blocks_per_cycle: matches
                .value_of("cycle-size")
                .map(|s| s.parse::<usize>().unwrap())
                .unwrap(),
            backend: matches.value_of("backend").unwrap().to_string(),
            blocks_limit: matches
                .value_of("blocks_limit")
                .map(|s| s.parse::<usize>().unwrap()),
            output: matches.value_of("output").unwrap().to_string(),
            input: matches.value_of("input").unwrap().to_string(),
        }
    }
}

pub fn get_tree_action(action: &ContextAction) -> String {
    match action {
        ContextAction::Get { .. } => "ContextAction::Get".to_string(),
        ContextAction::Mem { .. } => "ContextAction::Mem".to_string(),
        ContextAction::DirMem { .. } => "ContextAction::DirMem".to_string(),
        ContextAction::Set { .. } => "ContextAction::Set".to_string(),
        ContextAction::Copy { .. } => "ContextAction::Copy".to_string(),
        ContextAction::Delete { .. } => "ContextAction::Delete".to_string(),
        ContextAction::RemoveRecursively { .. } => "ContextAction::RemoveRecursively".to_string(),
        ContextAction::Commit { .. } => "ContextAction::Commit".to_string(),
        ContextAction::Fold { .. } => "ContextAction::Fold".to_string(),
        ContextAction::Checkout { .. } => "ContextAction::Checkout".to_string(),
        ContextAction::Shutdown { .. } => "ContextAction::Shutdown".to_string(),
    }
}

fn create_logger() -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
            .build()
            .fuse(),
    )
    .chan_size(32768)
    .overflow_strategy(slog_async::OverflowStrategy::Block)
    .build()
    .filter_level(Level::Debug)
    .fuse();

    Logger::root(drain, slog::o!())
}

// process actionfile without deselializing blocks
// in order to get count of blocks
fn get_blocks_count(log: &Logger, path: PathBuf) -> u32 {
    let mut counter = 0;
    let file = OpenOptions::new()
        .write(false)
        .create(false)
        .read(true)
        .open(path)
        .unwrap();
    let mut reader = BufReader::new(file);
    let mut pos = 0_u64;

    let mut block_size = [0_u8; 4];

    loop {
        if reader.seek(SeekFrom::Start(pos)).is_err() {
            warn!(log, "missing block operations information");
            break;
        }
        if reader.read_exact(&mut block_size).is_err() {
            break;
        }
        // skips header
        pos += block_size.len() as u64;
        // skips block
        pos += u32::from_be_bytes(block_size) as u64;
        counter += 1;
    }
    counter
}

fn create_key_value_store(path: &PathBuf, cache: &Cache) -> Arc<rocksdb::DB> {
    let schemas = vec![
        storage::block_storage::BlockPrimaryIndex::descriptor(&cache),
        storage::block_storage::BlockByLevelIndex::descriptor(&cache),
        storage::block_storage::BlockByContextHashIndex::descriptor(&cache),
        storage::BlockMetaStorage::descriptor(&cache),
        storage::OperationsStorage::descriptor(&cache),
        storage::OperationsMetaStorage::descriptor(&cache),
        storage::context_action_storage::ContextActionByBlockHashIndex::descriptor(&cache),
        storage::context_action_storage::ContextActionByContractIndex::descriptor(&cache),
        storage::context_action_storage::ContextActionByTypeIndex::descriptor(&cache),
        storage::ContextActionStorage::descriptor(&cache),
        storage::merkle_storage::MerkleStorage::descriptor(&cache),
        storage::SystemStorage::descriptor(&cache),
        storage::persistent::sequence::Sequences::descriptor(&cache),
        storage::MempoolStorage::descriptor(&cache),
        storage::ChainMetaStorage::descriptor(&cache),
        storage::PredecessorStorage::descriptor(&cache),
    ];

    let db_config = storage::persistent::DbConfiguration::default();
    storage::persistent::open_kv(path, schemas, &db_config)
        .map(Arc::new)
        .unwrap()
}

struct StatsWriter {
    output: File,
    block_latencies_total: usize,
    merkle_actions: Vec<MerkleStorageAction>,
}

impl StatsWriter {
    fn new(output: PathBuf) -> Self {
        let mut rv = Self {
            output: File::create(output.to_str().unwrap()).unwrap(),
            block_latencies_total: 0,
            merkle_actions: vec![
                MerkleStorageAction::Set,
                MerkleStorageAction::Get,
                MerkleStorageAction::GetByPrefix,
                MerkleStorageAction::GetKeyValuesByPrefix,
                MerkleStorageAction::GetContextTreeByPrefix,
                MerkleStorageAction::GetHistory,
                MerkleStorageAction::Mem,
                MerkleStorageAction::DirMem,
                MerkleStorageAction::Copy,
                MerkleStorageAction::Delete,
                MerkleStorageAction::DeleteRecursively,
                MerkleStorageAction::Commit,
                MerkleStorageAction::Checkout,
                MerkleStorageAction::BlockApplied,
            ],
        };
        rv.write_header();
        rv
    }

    fn generate_stats_for_merkle_action(
        &self,
        action: MerkleStorageAction,
        stats: &OperationLatencyStats,
    ) -> String {
        match stats.get(&action) {
            Some(v) => {
                format!(
                    "{} {} {} {}",
                    v.cumul_op_exec_time, v.avg_exec_time, v.op_exec_time_min, v.op_exec_time_max
                )
            }
            None => {
                format!("{} {} {} {}", 0, 0, 0, 0)
            }
        }
    }

    fn write_header(&mut self) {
        let header: String = self
            .merkle_actions
            .iter()
            .map(|action| match action {
                MerkleStorageAction::Set => "Set",
                MerkleStorageAction::Get => "Get",
                MerkleStorageAction::GetByPrefix => "GetByPrefix",
                MerkleStorageAction::GetKeyValuesByPrefix => "GetKeyValuesByPrefix",
                MerkleStorageAction::GetContextTreeByPrefix => "GetContextTreeByPrefix",
                MerkleStorageAction::GetHistory => "GetHistory",
                MerkleStorageAction::Mem => "Mem",
                MerkleStorageAction::DirMem => "DirMem",
                MerkleStorageAction::Copy => "Copy",
                MerkleStorageAction::Delete => "Delete",
                MerkleStorageAction::DeleteRecursively => "DeleteRecursively",
                MerkleStorageAction::Commit => "Commit",
                MerkleStorageAction::Checkout => "Checkout",
                MerkleStorageAction::BlockApplied => "BlockApplied",
            })
            .map(|action| {
                format!(
                    "{}_total {}_avg {}_min {}_max ",
                    action, action, action, action
                )
            })
            .collect();

        writeln!(&mut self.output, "block mem block_latency time {}", header).unwrap();
        self.output.flush().unwrap();
    }

    fn update(&mut self, block_nr: usize, merkle: Arc<RwLock<MerkleStorage>>) {
        let m = merkle.read().unwrap();

        let report = m.get_merkle_stats().unwrap();
        let usage = report.kv_store_stats;
        let block_latency = m.get_block_latency(0).unwrap();
        self.block_latencies_total += block_latency as usize;

        let stats: String = format!(
            "{} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} ",
            block_nr,
            usage,
            block_latency,
            self.block_latencies_total,
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::Set,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::Get,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::GetByPrefix,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::GetKeyValuesByPrefix,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::GetContextTreeByPrefix,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::GetHistory,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::Mem,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::DirMem,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::Copy,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::Delete,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::DeleteRecursively,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::Commit,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::Checkout,
                &report.perf_stats.global
            ),
            self.generate_stats_for_merkle_action(
                MerkleStorageAction::BlockApplied,
                &report.perf_stats.global
            )
        );

        writeln!(&mut self.output, "{}", stats).unwrap();
    }
}

fn main() -> Result<(), Error> {
    let params = Args::read_args();

    let out_dir = PathBuf::from(params.output.as_str());
    let key_value_db_path = out_dir.join("key_value_store");

    let actions_storage_path = PathBuf::from(params.input.as_str());

    let _ = fs::remove_dir_all(&out_dir)?;
    let _ = fs::create_dir(&out_dir)?;

    let logger = create_logger();

    let kvbackend = match params.backend.as_str() {
        "rocksdb" => {
            let cache = Cache::new_lru_cache(128 * 1024 * 1024).unwrap(); // 128 MB
            let kv = create_key_value_store(&key_value_db_path, &cache);
            MerkleStorage::new(Box::new(RocksDBBackend::new(kv)))
        }
        "inmem" => MerkleStorage::new(Box::new(InMemoryBackend::new())),
        "sled" => {
            let sled = sled::Config::new()
                .path(out_dir.join("sled"))
                .open()
                .unwrap();
            MerkleStorage::new(Box::new(SledBackend::new(sled)))
        }
        "btree" => MerkleStorage::new(Box::new(BTreeMapBackend::new())),
        "mark_sweep" => MerkleStorage::new(Box::new(MarkSweepGCed::<InMemoryBackend>::new(7))),
        "mark_move" => MerkleStorage::new(Box::new(MarkMoveGCed::<BTreeMapBackend>::new(7))),
        _ => panic!("unknown backend"),
    };

    let merkle = Arc::new(RwLock::new(kvbackend));

    let mut context: Box<dyn ContextApi> = Box::new(TezedgeContext::new(None, merkle.clone()));

    let mut stat_writer = StatsWriter::new(out_dir.join(params.backend + ".txt"));

    info!(
        logger,
        "Reading info from file {}",
        actions_storage_path.to_str().unwrap()
    );

    let mut counter = 0;
    let mut cycle_counter = 0;
    let blocks_count = get_blocks_count(&logger, actions_storage_path.clone());

    info!(logger, "{} blocks found", blocks_count);

    let actions_reader = ActionsFileReader::new(&actions_storage_path).unwrap();

    for messages in actions_reader.take(params.blocks_limit.unwrap_or(blocks_count as usize)) {
        counter += 1;
        let progress = counter as f64 / blocks_count as f64 * 100.0;

        for action in messages.iter() {
            match action {
                // actions that does not mutate staging area can be ommited here
                ContextAction::Set { .. }
                | ContextAction::Copy { .. }
                | ContextAction::Delete { .. }
                | ContextAction::RemoveRecursively { .. }
                | ContextAction::Commit { .. }
                | ContextAction::Checkout { .. }
                | ContextAction::Get { .. }
                | ContextAction::Mem { .. }
                | ContextAction::DirMem { .. }
                | ContextAction::Fold { .. } => {
                    if let Err(e) = perform_context_action(&action, &mut context) {
                        panic!("cannot perform action error: '{}'", e);
                    }
                }
                ContextAction::Shutdown { .. } => {}
            };

            // verify state of the storage after action has been applied
            match action {
                ContextAction::Commit {
                    new_context_hash, ..
                } => {
                    assert_eq!(
                        new_context_hash.clone(),
                        context.get_last_commit_hash().unwrap()
                    );
                }
                ContextAction::Checkout { context_hash, .. } => {
                    assert!(!context_hash.is_empty());
                    assert_eq!(
                        context_hash.clone(),
                        context.get_last_commit_hash().unwrap()
                    );
                }
                ContextAction::Get { key, value, .. } => {
                    assert_eq!(value.clone(), context.get_key(key).unwrap());
                }
                ContextAction::Mem { key, value, .. } => {
                    assert_eq!(*value, context.mem(key).unwrap());
                }
                ContextAction::DirMem { key, value, .. } => {
                    assert_eq!(*value, context.dirmem(key).unwrap());
                }
                _ => {}
            };

            // verify context hashes after each block
            if let Some(expected_hash) = get_new_tree_hash(&action) {
                assert_eq!(context.get_merkle_root(), expected_hash);
            }

            if let ContextAction::Commit { block_hash, .. } = &action {
                debug!(
                        logger,
                        "progress {:.7}% - cycle nr: {} block nr {} [{}] with {} messages processed - {} mb",
                        progress,
                        cycle_counter,
                        counter,
                        hex::encode(&block_hash.clone().unwrap().clone()),
                        messages.len(),
                            merkle.clone()
                            .read()
                            .unwrap()
                            .get_memory_usage()
                            .unwrap()
                            / 1024
                            / 1024
                    );

                context.block_applied().unwrap();
                if counter > 0 && counter % params.blocks_per_cycle == 0 {
                    context.cycle_started().unwrap();
                    cycle_counter += 1;
                }
            }
        }
        stat_writer.update(counter, merkle.clone());
    }
    Ok(())
}
