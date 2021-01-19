use std::{fs, path::PathBuf, sync::Arc, thread, time::Duration};

use failure::Error;
use riker::actors::SystemBuilder;
use rocksdb::Cache;
use shell::context_listener::{ContextListener, perform_context_action};
use slog::{Drain, Level, Logger, crit, debug, error, info};
use storage::{BlockStorage, context::{ContextApi, TezedgeContext}, persistent::{CommitLogSchema, CommitLogs, KeyValueSchema, PersistentStorage}};
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

fn get_new_tree_hash(action: &ContextAction) -> Option<[u8;32]> {
    let result = match &action{
        ContextAction::Set {new_tree_hash, ..} => {Some(new_tree_hash.clone())}
        ContextAction::Copy {new_tree_hash, ..} => {Some(new_tree_hash.clone())}
        ContextAction::Delete {new_tree_hash, ..} => {Some(new_tree_hash.clone())}
        ContextAction::RemoveRecursively {new_tree_hash, ..} => {Some(new_tree_hash.clone())}
        _ => {None}
    };
    match result {
        Some(v) => {
            if v.len() != 32 {
                None
            }else{
                let mut hash:[u8;32] = [0;32];
                // TODO: probably there is a more clever way to do that
                for (h,d) in hash.iter_mut().zip(v){
                    *h = d;
                }
                Some(hash)
            }
        }
        None => {None}
    }
}

#[test]
fn feed_tezedge_context_with_Actions() -> Result<(), Error> {

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
    let merkle = storage.merkle();
    let mut context: Box<dyn ContextApi> = Box::new(TezedgeContext::new(
        BlockStorage::new(&storage),
        storage.merkle(),
        false
    ));

    let actions_reader = io::ActionsFileReader::new(&actions_storage_path).unwrap();
    info!(logger, "Reading info from file {}", actions_storage_path.to_str().unwrap());
    info!(logger, "{}", actions_reader.header());
    for (block,actions) in actions_reader{
        debug!(logger, "processing block hash:{:?} with {} actions", &block, actions.len());
        
        for (_, action) in actions.iter().enumerate(){
            // TODO convert actions
            let action = tezos_context::channel::ContextAction::Shutdown;
            let result = perform_context_action(&action, & mut context);
            assert_eq!(true, result.is_ok());

            if let Some(hash) = get_new_tree_hash(&action){
                assert_eq!(merkle.read().unwrap().get_staged_root_hash(), hash)
            }
            

        }

    }
    
    Ok(())
}