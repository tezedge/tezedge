//! Temporary file.
//!
//! Used to initialize persistent storage.

use rocksdb::DB;
use std::convert::TryInto;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use storage::{
    database::tezedge_database::{TezedgeDatabase, TezedgeDatabaseBackendConfiguration},
    initializer::{
        DbsRocksDbTableInitializer, GlobalRocksDbCacheHolder, MainChain, RocksDbCache,
        RocksDbColumnFactory, RocksDbConfig,
    },
    persistent::{
        database::open_kv, open_cl, open_main_db, sequence::Sequences, CommitLogSchema, DBError,
        DbConfiguration,
    },
    BlockHeaderWithHash, BlockStorage, PersistentStorage,
};

use crypto::hash::BlockHash;
use tezos_messages::p2p::binary_message::{BinaryRead, MessageHash};
use tezos_messages::p2p::encoding::block_header::{BlockHeader, BlockHeaderBuilder, Fitness};

pub fn initialize_rocksdb<Factory: RocksDbColumnFactory>(
    config: &RocksDbConfig<Factory>,
) -> Result<Arc<DB>, DBError> {
    let kv_cache = RocksDbCache::new_lru_cache(config.cache_size)
        .expect("Failed to initialize RocksDB cache (db)");

    let db = open_kv(
        &config.db_path,
        config.columns.create(&kv_cache),
        &DbConfiguration {
            max_threads: config.threads,
        },
    )
    .map(Arc::new)?;

    Ok(db)
}

fn initialize_maindb<C: RocksDbColumnFactory>(
    kv: Option<Arc<DB>>,
    config: &RocksDbConfig<C>,
    log: slog::Logger,
) -> Arc<TezedgeDatabase> {
    Arc::new(
        open_main_db(
            kv,
            config,
            TezedgeDatabaseBackendConfiguration::RocksDB,
            log,
        )
        .expect("Failed to create/initialize MainDB database (db)"),
    )
}

pub fn init_storage() -> PersistentStorage {
    let config = RocksDbConfig {
        cache_size: 1024 * 1024,
        expected_db_version: 20,
        db_path: PathBuf::from("./data/db"),
        columns: DbsRocksDbTableInitializer,
        threads: Some(4),
    };

    let log = slog::Logger::root(slog::Discard, slog::o!());

    let maindb = {
        let kv =
            initialize_rocksdb(&config).expect("Failed to create/initialize RocksDB database (db)");
        initialize_maindb(Some(kv), &config, log.clone())
    };

    let commit_logs = Arc::new(
        open_cl(Path::new("./data"), vec![BlockStorage::descriptor()], log)
            .expect("Failed to open plain block_header storage"),
    );
    let sequences = Arc::new(Sequences::new(maindb.clone(), 1000));

    PersistentStorage::new(maindb, commit_logs, sequences)
}

pub fn gen_block_headers() -> Vec<BlockHeaderWithHash> {
    let mut builder = BlockHeaderBuilder::default();
    builder
        .level(34)
        .proto(1)
        .predecessor(
            "BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET"
                .try_into()
                .unwrap(),
        )
        .timestamp(5_635_634)
        .validation_pass(4)
        .operations_hash(
            "LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc"
                .try_into()
                .unwrap(),
        )
        .fitness(Fitness::new())
        .context(
            "CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd"
                .try_into()
                .unwrap(),
        )
        .protocol_data(vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);

    let header_1 = builder
        .clone()
        .level(1)
        .timestamp(5_635_634)
        .predecessor(
            "BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET"
                .try_into()
                .unwrap(),
        )
        .build()
        .unwrap();
    let header_1_hash: BlockHash = header_1.message_hash().unwrap().try_into().unwrap();

    let header_2 = builder
        .clone()
        .level(2)
        .timestamp(5_635_635)
        .predecessor(header_1_hash.clone())
        .build()
        .unwrap();
    let header_2_hash: BlockHash = header_2.message_hash().unwrap().try_into().unwrap();

    let header_3 = builder
        .clone()
        .level(3)
        .timestamp(5_635_636)
        .predecessor(header_2_hash.clone())
        .build()
        .unwrap();

    let header_3_hash: BlockHash = header_3.message_hash().unwrap().try_into().unwrap();

    vec![
        BlockHeaderWithHash {
            hash: header_1_hash,
            header: Arc::new(header_1),
        },
        BlockHeaderWithHash {
            hash: header_2_hash,
            header: Arc::new(header_2),
        },
        BlockHeaderWithHash {
            hash: header_3_hash,
            header: Arc::new(header_3),
        },
    ]
}
