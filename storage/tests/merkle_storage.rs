// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;
use std::error::Error;
use std::time::Instant;
use std::collections::VecDeque;
use std::sync::{mpsc, Arc};
use std::thread;
use serde::{Serialize, Deserialize};
use crypto::hash::{HashType, BlockHash};
use rocksdb::{DB, Cache, Options};

use storage::*;
use in_memory::KVStore;
use context_action_storage::ContextAction;
use merkle_storage::{MerkleStorage, MerkleError, Entry, EntryHash, check_commit_hashes};
use persistent::{PersistentStorage, CommitLogSchema, DbConfiguration, KeyValueSchema, open_cl, open_kv};
use persistent::sequence::Sequences;

const OPEN_FILES_LIMIT: u64 = 64 * 1024; //64k open files limit for process

// fn get_cycles_for_block(persistent_storage: &PersistentStorage, context_hash: &ContextHash) -> i32 {
//     let tezedge_context = TezedgeContext::new(
//         // BlockStorage::new(&persistent_storage),
//         BlockStorage::new(persistent_storage),
//         persistent_storage.merkle(),
//     );
//     let protocol_hash = tezedge_context.get_key_from_history(&context_hash, &context_key!("protocol")).unwrap();
//     let constants_data = tezedge_context.get_key_from_history(&context_hash, &context_key!("data/v1/constants")).unwrap();
//     let constants = tezos_messages::protocol::get_constants_for_rpc(&constants_data, protocol_hash).unwrap().unwrap();

//     match constants.get("blocks_per_cycle") {
//         Some(UniversalValue::Number(value)) => *value,
//         _ => panic!(4096),
//     }
// }

// Sets the limit of open file descriptors for the process
// If user set a higher limit before, it will be left as is
unsafe fn set_file_desc_limit(num: u64) {
    // Get current open file desc limit
    let rlim = match rlimit::getrlimit(rlimit::Resource::NOFILE) {
        Ok(rlim) => rlim,
        Err(e) => {
            eprintln!("Setting open files limit failed (getrlimit): {}", e);
            return;
        }
    };
    let (mut soft, hard) = rlim;
    // If the currently set rlimit is higher, do not change it
    if soft >= num {
        return;
    }
    // Set rlimit to num, but not higher than hard limit
    soft = num.min(hard);
    match rlimit::setrlimit(rlimit::Resource::NOFILE, soft, hard) {
        Ok(()) => println!("Open files limit set to {}.", soft),
        Err(e) => eprintln!("Setting open files limit failed (setrlimit): {}", e),
    }
}

fn init_persistent_storage() -> PersistentStorage {
    unsafe { set_file_desc_limit(OPEN_FILES_LIMIT); };
    // Parses config + cli args
    // let env = crate::configuration::Environment::from_args();
    let db_path = "/tmp/tezedge/light-node";

    // create common RocksDB block cache to be shared among column families
    // IMPORTANT: Cache object must live at least as long as DB (returned by open_kv)
    let cache = Cache::new_lru_cache(128 * 1024 * 1024).unwrap(); // 128 MB

    let schemas = vec![
        block_storage::BlockPrimaryIndex::descriptor(&cache),
        block_storage::BlockByLevelIndex::descriptor(&cache),
        block_storage::BlockByContextHashIndex::descriptor(&cache),
        BlockMetaStorage::descriptor(&cache),
        OperationsStorage::descriptor(&cache),
        OperationsMetaStorage::descriptor(&cache),
        context_action_storage::ContextActionByBlockHashIndex::descriptor(&cache),
        context_action_storage::ContextActionByContractIndex::descriptor(&cache),
        context_action_storage::ContextActionByTypeIndex::descriptor(&cache),
        ContextActionStorage::descriptor(&cache),
        SystemStorage::descriptor(&cache),
        Sequences::descriptor(&cache),
        MempoolStorage::descriptor(&cache),
        ChainMetaStorage::descriptor(&cache),
        PredecessorStorage::descriptor(&cache),
        MerkleStorage::descriptor(&cache),
    ];

    let opts = DbConfiguration::default();
    let rocks_db = Arc::new(open_kv(db_path, schemas, &opts).unwrap());
    let commit_logs = match open_cl(db_path, vec![BlockStorage::descriptor()]) {
        Ok(commit_logs) => Arc::new(commit_logs),
        Err(e) => panic!(e),
    };

    PersistentStorage::new(rocks_db, commit_logs)
}

struct BlocksIterator {
    block_storage: BlockStorage,
    blocks: std::vec::IntoIter<BlockHeaderWithHash>,
    next_block_chunk_hash: Option<BlockHash>,
    limit: usize,
}

impl BlocksIterator {
    pub fn new(block_storage: BlockStorage, start_block_hash: &BlockHash, limit: usize) -> Self {
        let mut this = Self { block_storage, limit, next_block_chunk_hash: None, blocks: vec![].into_iter() };

        this.get_and_set_blocks(start_block_hash);
        this
    }

    fn get_and_set_blocks(&mut self, from_block_hash: &BlockHash) -> Result<(), ()> {
        let mut new_blocks = Self::get_blocks_after_block(
            &self.block_storage,
            from_block_hash,
            self.limit,
        );

        if new_blocks.len() == 0 {
            return Err(());
        }
        if new_blocks.len() >= self.limit {
            self.next_block_chunk_hash = Some(new_blocks.pop().unwrap().hash);
        }
        self.blocks = new_blocks.into_iter();

        Ok(())
    }

    fn get_blocks_after_block(
        block_storage: &BlockStorage,
        block_hash: &BlockHash,
        limit: usize
    ) -> Vec<BlockHeaderWithHash> {
        block_storage.get_multiple_without_json(block_hash, limit).unwrap_or(vec![])
    }
}

impl Iterator for BlocksIterator {
    type Item = BlockHeaderWithHash;

    fn next(&mut self) -> Option<Self::Item> {
        match self.blocks.next() {
            Some(block) => Some(block),
            None => {
                if let Some(next_block_chunk_hash) = self.next_block_chunk_hash.take() {
                    if self.get_and_set_blocks(&next_block_chunk_hash).is_ok() {
                        return self.next();
                    }
                }
                None
            },
        }
    }
}

type BlockAndActions = (BlockHeaderWithHash, Vec<ContextAction>);

fn recv_blocks(
    block_storage: BlockStorage,
    ctx_action_storage: ContextActionStorage,
) -> impl Iterator<Item = BlockAndActions> {
    let genesis_block_hash = HashType::BlockHash.b58check_to_hash("BLockGenesisGenesisGenesisGenesisGenesis355e8bjkYPv").unwrap();
    let (tx, rx) = mpsc::sync_channel(128);

    thread::spawn(move || {
        for block in BlocksIterator::new(block_storage, &genesis_block_hash, 8) {
            let mut actions = ctx_action_storage.get_by_block_hash(&block.hash).unwrap();
            actions.sort_by_key(|x| x.id);
            let actions = actions.into_iter().map(|x| x.action).collect();
            if let Err(_) = tx.send((block, actions)) {
                dbg!("blocks receiver disconnected, shutting down sender thread");
                return;
            }
        }
    });

    rx.into_iter()
}

#[test]
fn test_merkle_storage_gc() {
    let persistent_storage = init_persistent_storage();

    let preserved_cycles = 5usize;
    let mut cycle_commit_hashes:Vec<Vec<EntryHash>> = vec![Default::default(); preserved_cycles - 1];

    let block_storage = BlockStorage::new(&persistent_storage);
    let ctx_action_storage = ContextActionStorage::new(&persistent_storage);
    let merkle_rwlock = persistent_storage.merkle();
    let mut merkle = merkle_rwlock.write().unwrap();

    for (block, actions) in recv_blocks(block_storage, ctx_action_storage) {
        println!("applying block: {}", block.header.level());

        let t = Instant::now();
        let actions_len = actions.len();

        for action in actions.into_iter() {
            if let ContextAction::Commit { new_context_hash, .. } = &action {
                cycle_commit_hashes.last_mut().unwrap().push(
                    new_context_hash[..].try_into().unwrap()
                );
            }
            merkle.apply_context_action(&action).unwrap();
        }

        println!("applied actions of block: {}, actions: {}, duration: {}ms", block.header.level(), actions_len, t.elapsed().as_millis());

        let (level, context_hash) = (block.header.level(), block.header.context());
        // let cycles = get_cycles_for_block(&persistent_storage, &context_hash);
        let cycles = 2048;

        if level % cycles == 0 && level > 0 {
            merkle.start_new_cycle().unwrap();
            println!("started new cycle: {}", level / cycles + 1);

            let t = Instant::now();
            merkle.wait_for_gc_finish();
            println!("waited for GC: {}ms", t.elapsed().as_millis());
            let t = Instant::now();
            let commits_iter = cycle_commit_hashes.iter()
                .flatten()
                .cloned();
            check_commit_hashes(&merkle, commits_iter).unwrap();
            println!("Block integrity intact! duration: {}ms", t.elapsed().as_millis());

            cycle_commit_hashes = cycle_commit_hashes.into_iter()
                .skip(1)
                .chain(vec![vec![]])
                .collect();
        }
    }
}
