use std::{path::PathBuf, sync::Arc, thread, time::Duration};

use failure::Error;
use riker::actors::SystemBuilder;
use rocksdb::Cache;
use shell::context_listener::ContextListener;
use slog::{Drain, Level, Logger};
use storage::{BlockStorage, persistent::{CommitLogSchema, CommitLogs, KeyValueSchema, PersistentStorage}};
use tezos_context::channel::ContextAction;
use tezos_wrapper::service::{IpcEvtServer, NoopMessage};
use ipc::IpcClient;

fn create_logger() -> Logger{
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

fn create_commit_log(path: &PathBuf) -> Arc<CommitLogs>{
    storage::persistent::open_cl(&path, vec![BlockStorage::descriptor()])
        .map(Arc::new)
        .unwrap()
}

fn create_key_value_store(path: &PathBuf, cache: &Cache) -> Arc<rocksdb::DB>{
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

fn read_context_actions_from_file(path: &PathBuf) -> Vec<ContextAction> {
    vec![]
}

#[test]
fn block_header_with_hash_encoded_equals_decoded() -> Result<(), Error> {

    let cache = Cache::new_lru_cache(128 * 1024 * 1024).unwrap(); // 128 MB
    let commit_log_db_path = PathBuf::from("/tmp/commit_log/");
    let key_value_db_path = PathBuf::from("/tmp/key_value_store/");

    let logger = create_logger();
    let commit_log = create_commit_log(&commit_log_db_path);
    let key_value_store = create_key_value_store(&key_value_db_path, &cache);
    let storage = PersistentStorage::new(key_value_store.clone(), commit_log.clone());
    
    let event_server = IpcEvtServer::try_new().unwrap();
    let socket_path = event_server.socket_path();
    let sys = SystemBuilder::new()
        .name("block_header_with_hash_encoded_equals_decoded")
        .log(logger.clone())
        .create()
        .unwrap();

    //spawns actions receiver thread
    let _actor = ContextListener::actor(&sys, &storage, event_server, logger, false).unwrap();

    

    let client:  IpcClient<NoopMessage, ContextAction> = IpcClient::new(&socket_path);
    let (mut rx, mut tx) = client.connect().unwrap();

    for action in read_context_actions_from_file(&PathBuf::from("/tmp/actions.bin")).iter(){
        tx.send(action).unwrap();
        rx.receive().unwrap();
    }

    tx.send(&ContextAction::Shutdown{}).unwrap();
    Ok(())
}
