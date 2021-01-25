use std::{fs, path::PathBuf, sync::Arc, thread, time::Duration};

use failure::Error;
use riker::actors::SystemBuilder;
use rocksdb::Cache;
use shell::context_listener::ContextListener;
use slog::{Drain, Level, Logger, debug, error, info};
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

#[test]
fn block_header_with_hash_encoded_equals_decoded() -> Result<(), Error> {

    let cache = Cache::new_lru_cache(128 * 1024 * 1024).unwrap(); // 128 MB
    let commit_log_db_path = PathBuf::from("/tmp/commit_log/");
    let key_value_db_path = PathBuf::from("/tmp/key_value_store/");
    let actions_storage_path = PathBuf::from("/mnt/hdd/actions.bin");

    let _ = fs::remove_dir_all(&commit_log_db_path);
    let _ = fs::remove_dir_all(&key_value_db_path);

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

    
    info!(logger, "initial commit hash");
    //spawns actions receiver thread
    let _actor = ContextListener::actor(&sys, &storage, event_server, logger.clone(), false).unwrap();
    
    

    let client:  IpcClient<NoopMessage, io::channel::ContextAction> = IpcClient::new(&socket_path);
    let (_, mut tx) = client.connect().unwrap();

    let actions_reader = io::ActionsFileReader::new(&actions_storage_path).unwrap();
    info!(logger, "Reading info from file {}", actions_storage_path.to_str().unwrap());
    info!(logger, "{}", actions_reader.header());
    for (block,actions) in actions_reader{
        debug!(logger, "processing block hash:{:?} with {} actions", &block, actions.len());

        for (_, action) in actions.iter().enumerate(){
            if let Err(e) =tx.send(action){
                error!(logger,"reason => {}", e);
                tx.send(&io::channel::ContextAction::Shutdown).unwrap();
                thread::sleep(Duration::from_secs(1));
                return Ok(());
            }
        }

        // loop{
        //     if let Some(hash) = storage.merkle().read().unwrap().get_last_commit_hash(){
        //         info!(logger,"found merkle root hash {:?}", &hash);
        //         break;
        //     };
        //     thread::sleep(Duration::from_millis(100));
        // }

    }


    
    Ok(())
}
