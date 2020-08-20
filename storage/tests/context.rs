// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crypto::hash::{ContextHash, HashType};
use storage::{BlockHeaderWithHash, BlockStorage};
use storage::context::{ContextApi, ContextIndex, TezedgeContext};
use storage::skip_list::Bucket;
use storage::tests_common::TmpStorage;
use tezos_messages::p2p::encoding::prelude::BlockHeaderBuilder;

#[test]
pub fn test_context_set_get_commit() -> Result<(), failure::Error> {
    // prepare temp storage
    let tmp_storage = TmpStorage::create(test_storage_dir_path("__context:test_context_set_get_commit")).expect("Storage error");
    let persistent_storage = tmp_storage.storage();

    // init block storage (because of commit)
    let block = dummy_block("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe", 0)?;
    let block_storage = BlockStorage::new(&persistent_storage);
    block_storage.put_block_header(&block)?;

    // context
    let mut context = TezedgeContext::new(
        BlockStorage::new(&persistent_storage),
        persistent_storage.context_storage(),
    );

    // add to context
    let mut diff = context.init_from_start();
    diff.set(&None, &to_key(["data", "rolls", "owner", "current", "index", "123"].to_vec()), &vec![1, 2, 3, 4, 5, 6])?;

    // commit
    let new_context_hash: ContextHash = HashType::ContextHash.string_to_bytes("CoV16kW8WgL51SpcftQKdeqc94D6ekghMgPMmEn7TSZzFA697PeE")?;

    context.commit(
        &block.hash,
        &None,
        &new_context_hash,
        &diff,
    )?;

    // get key from new commit
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "index", "123"], new_context_hash, Bucket::Exists(vec![1, 2, 3, 4, 5, 6]));

    Ok(())
}

#[test]
pub fn test_context_delete_and_remove() -> Result<(), failure::Error> {
    // prepare temp storage
    let tmp_storage = TmpStorage::create(test_storage_dir_path("__context:test_context_delete_and_remove")).expect("Storage error");
    let persistent_storage = tmp_storage.storage();

    // init block with level 0 (because of commit)
    let block = dummy_block("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe", 0)?;
    let block_storage = BlockStorage::new(&persistent_storage);
    block_storage.put_block_header(&block)?;

    // context
    let mut context = TezedgeContext::new(
        BlockStorage::new(&persistent_storage),
        persistent_storage.context_storage(),
    );

    // add to context
    let mut context_diff = context.init_from_start();
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu"].to_vec()), &vec![1, 2, 3])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "0"].to_vec()), &vec![1, 2, 3, 4])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "1"].to_vec()), &vec![1, 2, 3, 4, 5])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "1", "a"].to_vec()), &vec![1, 2, 3, 4, 5])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "1", "b"].to_vec()), &vec![1, 2, 3, 4, 5])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "2"].to_vec()), &vec![1, 2, 3, 4, 5, 6])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "2", "a"].to_vec()), &vec![1, 2, 3, 4, 5, 61])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "2", "b"].to_vec()), &vec![1, 2, 3, 4, 5, 62])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "index", "123"].to_vec()), &vec![1, 2, 3, 4, 5, 6, 7])?;

    // commit
    let context_hash_1: ContextHash = HashType::ContextHash.string_to_bytes("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd")?;

    context.commit(
        &block.hash,
        &None,
        &context_hash_1,
        &context_diff,
    )?;

    // get key from new commit
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "0"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "1"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "1", "a"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "1", "b"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 6]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2", "a"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 61]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2", "b"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 62]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "index", "123"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 6, 7]));

    // insert another block with level 1
    let block = dummy_block("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET", 1)?;
    let block_storage = BlockStorage::new(&persistent_storage);
    block_storage.put_block_header(&block)?;

    // checkout last commit to be modified
    let mut context_diff = context.checkout(&context_hash_1)?;

    // 1. remove rec
    context.remove_recursively_to_diff(
        &Some(context_hash_1.clone()),
        &to_key(["data", "rolls", "owner", "current", "cpu", "2"].to_vec()),
        &mut context_diff,
    )?;
    context.delete_to_diff(
        &Some(context_hash_1.clone()),
        &to_key(["data", "rolls", "owner", "current", "cpu", "1", "b"].to_vec()),
        &mut context_diff,
    )?;

    // commit
    let context_hash_2: ContextHash = HashType::ContextHash.string_to_bytes("CoV16kW8WgL51SpcftQKdeqc94D6ekghMgPMmEn7TSZzFA697PeE")?;

    context.commit(
        &block.hash,
        &Some(context_hash_1),
        &context_hash_2,
        &context_diff,
    )?;

    // get key from new commit
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "0"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "1"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "1", "a"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "1", "b"], context_hash_2.clone(), Bucket::Deleted);
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2"], context_hash_2.clone(), Bucket::Deleted);
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2", "a"], context_hash_2.clone(), Bucket::Deleted);
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2", "b"], context_hash_2.clone(), Bucket::Deleted);
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "index", "123"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 6, 7]));

    Ok(())
}

#[test]
pub fn test_context_copy() -> Result<(), failure::Error> {
    // prepare temp storage
    let tmp_storage = TmpStorage::create(test_storage_dir_path("__context:context_copy")).expect("Storage error");
    let persistent_storage = tmp_storage.storage();

    // init block with level 0 (because of commit)
    let block = dummy_block("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe", 0)?;
    let block_storage = BlockStorage::new(&persistent_storage);
    block_storage.put_block_header(&block)?;

    // context
    let mut context = TezedgeContext::new(
        BlockStorage::new(&persistent_storage),
        persistent_storage.context_storage(),
    );

    // add to context
    let mut context_diff = context.init_from_start();
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu"].to_vec()), &vec![1, 2, 3])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "0"].to_vec()), &vec![1, 2, 3, 4])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "1"].to_vec()), &vec![1, 2, 3, 4, 5])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "2"].to_vec()), &vec![1, 2, 3, 4, 5, 6])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "2", "a"].to_vec()), &vec![1, 2, 3, 4, 5, 61])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "cpu", "2", "b"].to_vec()), &vec![1, 2, 3, 4, 5, 62])?;
    context_diff.set(&None, &to_key(["data", "rolls", "owner", "current", "index", "123"].to_vec()), &vec![1, 2, 3, 4, 5, 6, 7])?;

    // commit
    let context_hash_1: ContextHash = HashType::ContextHash.string_to_bytes("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd")?;

    context.commit(
        &block.hash,
        &None,
        &context_hash_1,
        &context_diff,
    )?;

    // get key from new commit
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "0"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "1"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 6]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2", "a"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 61]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2", "b"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 62]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "index", "123"], context_hash_1.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 6, 7]));

    // insert another block with level 1
    let block = dummy_block("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET", 1)?;
    let block_storage = BlockStorage::new(&persistent_storage);
    block_storage.put_block_header(&block)?;

    // checkout last commit to be modified
    let mut context_diff = context.checkout(&context_hash_1)?;

    // 1. copy
    context.copy_to_diff(
        &Some(context_hash_1.clone()),
        &to_key(["data", "rolls", "owner", "current"].to_vec()),
        &to_key(["data", "rolls", "owner", "snapshot", "01", "02"].to_vec()),
        &mut context_diff,
    )?;

    // commit
    let context_hash_2: ContextHash = HashType::ContextHash.string_to_bytes("CoV16kW8WgL51SpcftQKdeqc94D6ekghMgPMmEn7TSZzFA697PeE")?;

    context.commit(
        &block.hash,
        &Some(context_hash_1),
        &context_hash_2,
        &context_diff,
    )?;

    // get key from new commit - original stays unchanged
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "0"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "1"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 6]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2", "a"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 61]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "cpu", "2", "b"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 62]));
    assert_data_eq!(context, ["data", "rolls", "owner", "current", "index", "123"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 6, 7]));

    // get key from new commit - original stays unchanged
    assert_data_eq!(context, ["data", "rolls", "owner", "snapshot", "01", "02", "cpu"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3]));
    assert_data_eq!(context, ["data", "rolls", "owner", "snapshot", "01", "02", "cpu", "0"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4]));
    assert_data_eq!(context, ["data", "rolls", "owner", "snapshot", "01", "02", "cpu", "1"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5]));
    assert_data_eq!(context, ["data", "rolls", "owner", "snapshot", "01", "02", "cpu", "2"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 6]));
    assert_data_eq!(context, ["data", "rolls", "owner", "snapshot", "01", "02", "cpu", "2", "a"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 61]));
    assert_data_eq!(context, ["data", "rolls", "owner", "snapshot", "01", "02", "cpu", "2", "b"], context_hash_2.clone(), Bucket::Exists(vec![1, 2, 3, 4, 5, 62]));
    assert_data_eq!(context, ["data", "rolls", "owner", "snapshot", "01", "02", "index", "123"], context_hash_2, Bucket::Exists(vec![1, 2, 3, 4, 5, 6, 7]));

    Ok(())
}

fn to_key(key: Vec<&str>) -> Vec<String> {
    key
        .into_iter()
        .map(|k| k.to_string())
        .collect()
}

fn dummy_block(block_hash: &str, level: i32) -> Result<BlockHeaderWithHash, failure::Error> {
    Ok(
        BlockHeaderWithHash {
            hash: HashType::BlockHash.string_to_bytes(block_hash)?,
            header: Arc::new(
                BlockHeaderBuilder::default()
                    .level(level)
                    .proto(0)
                    .predecessor(HashType::BlockHash.string_to_bytes("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")?)
                    .timestamp(5_635_634)
                    .validation_pass(0)
                    .operations_hash(HashType::OperationListListHash.string_to_bytes("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc")?)
                    .fitness(vec![])
                    .context(HashType::ContextHash.string_to_bytes("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd")?)
                    .protocol_data(vec![])
                    .build().unwrap()
            ),
        }
    )
}

#[macro_export]
macro_rules! assert_data_eq {
    ($ctx:expr, $key:expr, $context_hash:expr, $data:expr) => {{
        let data = $ctx.get_key(&ContextIndex::new(None, Some($context_hash)), &to_key($key.to_vec()))?;
        assert!(data.is_some());
        assert_eq!(data.unwrap(), $data);
    }}
}

pub fn test_storage_dir_path(dir_name: &str) -> PathBuf {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    let path = Path::new(out_dir.as_str())
        .join(Path::new(dir_name))
        .to_path_buf();
    path
}