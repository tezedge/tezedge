use std::{fs, path::PathBuf, sync::Arc};

use rocksdb::Cache;
use failure::Error;
use shell::context_listener::perform_context_action;
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::prelude::{BlockHeaderBuilder};
use slog::{Drain, Level, Logger, debug, info};
use storage::{BlockStorage, context::{ContextApi, TezedgeContext}, persistent::{CommitLogSchema, CommitLogs, KeyValueSchema, PersistentStorage}};
use storage::action_file::ActionsFileReader;
use shell::context_listener::get_new_tree_hash;
use tezos_context::channel::ContextAction;

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

fn get_action_symbol(action: &ContextAction) -> String {
    match action{
        ContextAction::Set {..} => {String::from("Set")},
        ContextAction::Delete {..} => {String::from("Delete")},
        ContextAction::RemoveRecursively {..} => {String::from("RemoveRecursively")},
        ContextAction::Copy {..} => {String::from("Copy")},
        ContextAction::Checkout {..} => {String::from("Checkout")},
        ContextAction::Commit {..} => {String::from("Commit")},
        ContextAction::Mem {..} => {String::from("Mem")},
        ContextAction::DirMem {..} => {String::from("DirMem")},
        ContextAction::Get {..} => {String::from("Get")},
        ContextAction::Fold {..} => {String::from("Fold")},
        ContextAction::Shutdown {..} => {String::from("Shutdown")},
    }
}

#[test]
fn feed_tezedge_context_with_actions() -> Result<(), Error> {

    let block_header_stub = BlockHeaderBuilder::default()
        .level(0)
        .proto(0)
        .predecessor(vec![0;32])
        .timestamp(0)
        .validation_pass(0)
        .operations_hash(vec![0;32])
        .fitness(vec![])
        .context(vec![0;32])
        .protocol_data(vec![])
        .build()
        .unwrap();
    let header_stub = BlockHeaderWithHash::new(block_header_stub).unwrap();

    let cache = Cache::new_lru_cache(128 * 1024 * 1024).unwrap(); // 128 MB
    let commit_log_db_path = PathBuf::from("/tmp/commit_log/");
    let key_value_db_path = PathBuf::from("/tmp/key_value_store/");
    // let actions_storage_path = PathBuf::from("/mnt/hdd/milion.bin");
    let actions_storage_path = PathBuf::from("/mnt/hdd/node-data/actionfile.bin");

    let _ = fs::remove_dir_all(&commit_log_db_path);
    let _ = fs::remove_dir_all(&key_value_db_path);
    let logger = create_logger();

    let commit_log = create_commit_log(&commit_log_db_path);
    let key_value_store = create_key_value_store(&key_value_db_path, &cache);
    let storage = PersistentStorage::new(key_value_store.clone(), commit_log.clone());
    let mut context: Box<dyn ContextApi> = Box::new(TezedgeContext::new(
        BlockStorage::new(&storage),
        storage.merkle(),
    ));

    let block_storage = BlockStorage::new(&storage);

    let actions_reader = ActionsFileReader::new(&actions_storage_path).unwrap();
    info!(logger, "Reading info from file {}", actions_storage_path.to_str().unwrap());
    info!(logger, "{}", actions_reader.header());

    for messages in actions_reader{
        info!(logger, "HELLO");
        match messages.iter().last() {
            Some(msg) => {
                match &msg.action {
                    ContextAction::Commit{block_hash: Some(block_hash), ..} => {
                        debug!(logger, "processing block {} with {} messages", hex::encode(&block_hash), messages.len());
                    }
                    _ => {panic!("missing commit action")}
                }
            }
            None => {panic!("missing commit action")}
        };


        for msg in messages.iter(){

            if let ContextAction::Commit{block_hash: Some(block_hash), ..} = &msg.action{
                // there is extra validation in ContextApi::commit that verifies that 
                // applied action comes from known block - for testing purposes
                // block_storage needs to be fed with stub value in order to pass validation
               
                info!(logger, "commiting - this may take while");
                let mut b = header_stub.clone();
                b.hash = block_hash.clone();
                block_storage.put_block_header(&b).unwrap();

            }

            if let Err(e) = perform_context_action(&msg.action, & mut context){
                panic!("cannot perform action {:?} error: '{}'", &msg, e);
            }

            if let Some(expected_hash) = get_new_tree_hash(&msg.action){
                assert_eq!(context.get_merkle_root(), expected_hash);
            }
        }
    }
    Ok(())
}
    
