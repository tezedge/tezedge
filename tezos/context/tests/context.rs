// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use crypto::hash::ContextHash;
use storage::tests_common::TmpStorage;
use storage::{BlockHeaderWithHash, BlockStorage};
use tezos_context::initializer::{initialize_tezedge_context, ContextKvStoreConfiguration};
use tezos_context::{context_key, ContextError, TezedgeContext};
use tezos_context::{IndexApi, ProtocolContextApi, ShellContextApi};
use tezos_context_api::{
    ContextKey, TezosContextTezEdgeStorageConfiguration, TezosContextTezedgeOnDiskBackendOptions,
};
use tezos_messages::p2p::encoding::fitness::Fitness;
use tezos_messages::p2p::encoding::prelude::BlockHeaderBuilder;

#[test]
pub fn test_context_set_get_commit_persistent() -> Result<(), anyhow::Error> {
    context_set_get_commit(
        ContextKvStoreConfiguration::OnDisk(TezosContextTezedgeOnDiskBackendOptions {
            base_path: "".to_string(),
            startup_check: false,
        }),
        "__context:test_context_set_get_commit_persistent",
    )
}

#[test]
pub fn test_context_set_get_commit() -> Result<(), anyhow::Error> {
    context_set_get_commit(
        ContextKvStoreConfiguration::InMem(TezosContextTezedgeOnDiskBackendOptions {
            base_path: "".to_string(),
            startup_check: false,
        }),
        "__context:test_context_set_get_commit",
    )
}

pub fn context_set_get_commit(
    backend: ContextKvStoreConfiguration,
    tmp_dir: &str,
) -> Result<(), anyhow::Error> {
    // prepare temp storage
    let tmp_storage = TmpStorage::create_to_out_dir(tmp_dir).expect("Storage error");
    let persistent_storage = tmp_storage.storage();

    // init block storage (because of commit)
    let block = dummy_block("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe", 0)?;
    let block_storage = BlockStorage::new(persistent_storage);
    block_storage.put_block_header(&block)?;

    // context
    let mut context = initialize_tezedge_context(&TezosContextTezEdgeStorageConfiguration {
        backend,
        ipc_socket_path: None,
    })
    .unwrap();

    // add to context
    context = context.add(
        &context_key!("data/rolls/owner/current/index/123"),
        &[1, 2, 3, 4, 5, 6],
    )?;

    // commit
    let new_context_hash =
        ContextHash::try_from("CoVf53zSDGcSWS74Mxe2i2RJnVfCaMrAjxK2Xq7tgiFMtkNwUdPv")?;

    let hash = context.commit("Tezos".to_string(), "Genesis".to_string(), 0)?;
    assert_eq!(hash, new_context_hash);

    context
        .index
        .get_key_from_history(
            &new_context_hash,
            &context_key!("data/rolls/owner/current/index/123"),
        )
        .unwrap();

    // get key from new commit
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/index/123"),
        new_context_hash,
        vec![1, 2, 3, 4, 5, 6]
    );

    Ok(())
}

#[test]
pub fn test_context_hash_from_working_tree_persistent() -> Result<(), anyhow::Error> {
    context_hash_from_working_tree(
        ContextKvStoreConfiguration::OnDisk(TezosContextTezedgeOnDiskBackendOptions {
            base_path: "".to_string(),
            startup_check: false,
        }),
        "__context:test_context_hash_from_working_tree_persistent",
    )
}

#[test]
pub fn test_context_hash_from_working_tree_memory() -> Result<(), anyhow::Error> {
    context_hash_from_working_tree(
        ContextKvStoreConfiguration::InMem(TezosContextTezedgeOnDiskBackendOptions {
            base_path: "".to_string(),
            startup_check: false,
        }),
        "__context:test_context_hash_from_working_tree_memory",
    )
}

pub fn context_hash_from_working_tree(
    backend: ContextKvStoreConfiguration,
    tmp_dir: &str,
) -> Result<(), anyhow::Error> {
    // prepare temp storage
    let tmp_storage = TmpStorage::create_to_out_dir(tmp_dir).expect("Storage error");
    let persistent_storage = tmp_storage.storage();

    // init block storage (because of commit)
    let block = dummy_block("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe", 0)?;
    let block_storage = BlockStorage::new(persistent_storage);
    block_storage.put_block_header(&block)?;

    // context
    let mut context = initialize_tezedge_context(&TezosContextTezEdgeStorageConfiguration {
        backend,
        ipc_socket_path: None,
    })
    .unwrap();

    // Enough to create inodes
    for index in 0..1000 {
        context = context.add(
            &context_key!(format!("data/rolls/owner/current/index/{}", index)),
            &[1, 2, 3, 4, 5, 6],
        )?;
    }

    // Create 'temporary' hashes
    let hash = context
        .hash("Tezos".to_string(), "Genesis".to_string(), 0)
        .unwrap();

    // Created hashes must be commited, this must not panic
    let commit_hash = context
        .commit("Tezos".to_string(), "Genesis".to_string(), 0)
        .unwrap();

    assert_eq!(hash, commit_hash);

    let mut context = context.index.checkout(&commit_hash).unwrap().unwrap();

    // Enough to create inodes
    for index in 1000..1010 {
        context = context.add(
            &context_key!(format!("data/rolls/owner/current/index/{}", index)),
            &[1, 2, 3, 4, 5, 6],
        )?;
    }

    Ok(())
}

#[test]
pub fn test_context_delete_and_remove_persistent() -> Result<(), anyhow::Error> {
    context_delete_and_remove(
        ContextKvStoreConfiguration::OnDisk(TezosContextTezedgeOnDiskBackendOptions {
            base_path: "".to_string(),
            startup_check: false,
        }),
        "__context:test_context_delete_and_remove_persistent",
    )
}

#[test]
pub fn test_context_delete_and_remove() -> Result<(), anyhow::Error> {
    context_delete_and_remove(
        ContextKvStoreConfiguration::InMem(TezosContextTezedgeOnDiskBackendOptions {
            base_path: "".to_string(),
            startup_check: false,
        }),
        "__context:test_context_delete_and_remove",
    )
}

pub fn context_delete_and_remove(
    backend: ContextKvStoreConfiguration,
    tmp_dir: &str,
) -> Result<(), anyhow::Error> {
    // prepare temp storage
    let tmp_storage = TmpStorage::create_to_out_dir(tmp_dir).expect("Storage error");
    let persistent_storage = tmp_storage.storage();

    // init block with level 0 (because of commit)
    let block = dummy_block("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe", 0)?;
    let block_storage = BlockStorage::new(persistent_storage);
    block_storage.put_block_header(&block)?;

    // context
    let mut context = initialize_tezedge_context(&TezosContextTezEdgeStorageConfiguration {
        backend,
        ipc_socket_path: None,
    })
    .unwrap();

    // add to context
    context = context.add(
        &context_key!("data/rolls/owner/current/cpu/0"),
        &[1, 2, 3, 4],
    )?;
    context = context.add(
        &context_key!("data/rolls/owner/current/cpu/1/a"),
        &[1, 2, 3, 4, 5],
    )?;
    context = context.add(
        &context_key!("data/rolls/owner/current/cpu/1/b"),
        &[1, 2, 3, 4, 5],
    )?;
    context = context.add(
        &context_key!("data/rolls/owner/current/cpu/2/a"),
        &[1, 2, 3, 4, 5, 61],
    )?;
    context = context.add(
        &context_key!("data/rolls/owner/current/cpu/2/b"),
        &[1, 2, 3, 4, 5, 62],
    )?;
    context = context.add(
        &context_key!("data/rolls/owner/current/index/123"),
        &[1, 2, 3, 4, 5, 6, 7],
    )?;

    // commit
    let context_hash_1: ContextHash =
        "CoUyfscSjC3XYECq1aFYQQLrVZuNSW17B7SbFDV9W1REfhJpxZwB".try_into()?;

    let hash = context.commit("Tezos".to_string(), "Genesis".to_string(), 0)?;
    assert_eq!(hash, context_hash_1);

    // get key from new commit
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/0"),
        context_hash_1,
        vec![1, 2, 3, 4]
    );

    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/1/a"),
        context_hash_1,
        vec![1, 2, 3, 4, 5]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/1/b"),
        context_hash_1,
        vec![1, 2, 3, 4, 5]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/2/a"),
        context_hash_1,
        vec![1, 2, 3, 4, 5, 61]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/2/b"),
        context_hash_1,
        vec![1, 2, 3, 4, 5, 62]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/index/123"),
        context_hash_1,
        vec![1, 2, 3, 4, 5, 6, 7]
    );

    // insert another block with level 1
    let block = dummy_block("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET", 1)?;
    let block_storage = BlockStorage::new(persistent_storage);
    block_storage.put_block_header(&block)?;

    // checkout last commit to be modified
    context = context
        .index
        .checkout(&context_hash_1)?
        .unwrap_or_else(|| panic!("Commit not found: {}", context_hash_1.to_base58_check()));

    // 1. remove rec
    context = context.delete(&context_key!("data/rolls/owner/current/cpu/2"))?;
    context = context.delete(&context_key!("data/rolls/owner/current/cpu/1/b"))?;

    // commit
    let context_hash_2: ContextHash =
        "CoVGom58bpVjHWVsKuc8k7JC7QyzZ7n4ntGZiPpw2CwM43sxC4XF".try_into()?;

    let hash = context.commit("Tezos".to_string(), "Genesis".to_string(), 0)?;
    assert_eq!(hash, context_hash_2);

    // get key from new commit
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/0"),
        context_hash_2,
        vec![1, 2, 3, 4]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/1/a"),
        context_hash_2,
        vec![1, 2, 3, 4, 5]
    );
    assert_data_deleted!(
        context,
        context_key!("data/rolls/owner/current/cpu/1/b"),
        context_hash_2
    );
    assert_data_deleted!(
        context,
        context_key!("data/rolls/owner/current/cpu/2/a"),
        context_hash_2
    );
    assert_data_deleted!(
        context,
        context_key!("data/rolls/owner/current/cpu/2/b"),
        context_hash_2
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/index/123"),
        context_hash_2,
        vec![1, 2, 3, 4, 5, 6, 7]
    );

    Ok(())
}

fn ctx_copy(
    context: &TezedgeContext,
    from: &ContextKey,
    to: &ContextKey,
) -> Result<Option<TezedgeContext>, ContextError> {
    if let Some(tree) = context.find_tree(from)? {
        Ok(Some(context.add_tree(to, &tree)?))
    } else {
        Ok(None)
    }
}

#[test]
pub fn test_context_copy_persistent() -> Result<(), anyhow::Error> {
    context_copy(
        ContextKvStoreConfiguration::OnDisk(TezosContextTezedgeOnDiskBackendOptions {
            base_path: "".to_string(),
            startup_check: false,
        }),
        "__context:test_context_copy_persistent",
    )
}

#[test]
pub fn test_context_copy() -> Result<(), anyhow::Error> {
    context_copy(
        ContextKvStoreConfiguration::InMem(TezosContextTezedgeOnDiskBackendOptions {
            base_path: "".to_string(),
            startup_check: false,
        }),
        "__context:test_context_copy",
    )
}

fn context_copy(backend: ContextKvStoreConfiguration, tmp_dir: &str) -> Result<(), anyhow::Error> {
    // prepare temp storage
    let tmp_storage = TmpStorage::create_to_out_dir(tmp_dir).expect("Storage error");
    let persistent_storage = tmp_storage.storage();

    // init block with level 0 (because of commit)
    let block = dummy_block("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe", 0)?;
    let block_storage = BlockStorage::new(persistent_storage);
    block_storage.put_block_header(&block)?;

    // context
    let mut context = initialize_tezedge_context(&TezosContextTezEdgeStorageConfiguration {
        backend,
        ipc_socket_path: None,
    })
    .unwrap();

    // add to context
    context = context.add(
        &context_key!("data/rolls/owner/current/cpu/0"),
        &[1, 2, 3, 4],
    )?;
    context = context.add(
        &context_key!("data/rolls/owner/current/cpu/1"),
        &[1, 2, 3, 4, 5],
    )?;
    context = context.add(
        &context_key!("data/rolls/owner/current/cpu/2/a"),
        &[1, 2, 3, 4, 5, 61],
    )?;
    context = context.add(
        &context_key!("data/rolls/owner/current/cpu/2/b"),
        &[1, 2, 3, 4, 5, 62],
    )?;
    context = context.add(
        &context_key!("data/rolls/owner/current/index/123"),
        &[1, 2, 3, 4, 5, 6, 7],
    )?;

    // commit
    let context_hash_1: ContextHash =
        "CoVu1KaQQd2SFPqJh7go1t9q11upv1BewzShtTrNK7ZF6uCAcUQR".try_into()?;

    let hash = context.commit("Tezos".to_string(), "Genesis".to_string(), 0)?;
    assert_eq!(hash, context_hash_1);

    // get key from new commit
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/0"),
        context_hash_1,
        vec![1, 2, 3, 4]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/1"),
        context_hash_1,
        vec![1, 2, 3, 4, 5]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/2/a"),
        context_hash_1,
        vec![1, 2, 3, 4, 5, 61]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/2/b"),
        context_hash_1,
        vec![1, 2, 3, 4, 5, 62]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/index/123"),
        context_hash_1,
        vec![1, 2, 3, 4, 5, 6, 7]
    );

    // insert another block with level 1
    let block = dummy_block("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET", 1)?;
    let block_storage = BlockStorage::new(persistent_storage);
    block_storage.put_block_header(&block)?;

    // checkout last commit to be modified
    context = context
        .index
        .checkout(&context_hash_1)?
        .unwrap_or_else(|| panic!("Commit not found: {}", context_hash_1.to_base58_check()));

    // 1. copy
    let context = ctx_copy(
        &context,
        &context_key!("data/rolls/owner/current"),
        &context_key!("data/rolls/owner/snapshot/01/02"),
    )?
    .unwrap();

    // commit
    let context_hash_2: ContextHash =
        "CoVX1ptKigdesVSqaREXTTHKGegLGM4x1bSSFvPgX5V8qj85r98G".try_into()?;

    let hash = context.commit("Tezos".to_string(), "Genesis".to_string(), 0)?;
    assert_eq!(hash, context_hash_2);

    // get key from new commit - original stays unchanged
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/0"),
        context_hash_2,
        vec![1, 2, 3, 4]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/1"),
        context_hash_2,
        vec![1, 2, 3, 4, 5]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/2/a"),
        context_hash_2,
        vec![1, 2, 3, 4, 5, 61]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/cpu/2/b"),
        context_hash_2,
        vec![1, 2, 3, 4, 5, 62]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/current/index/123"),
        context_hash_2,
        vec![1, 2, 3, 4, 5, 6, 7]
    );

    // get key from new commit - original stays unchanged
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/snapshot/01/02/cpu/0"),
        context_hash_2,
        vec![1, 2, 3, 4]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/snapshot/01/02/cpu/1"),
        context_hash_2,
        vec![1, 2, 3, 4, 5]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/snapshot/01/02/cpu/2/a"),
        context_hash_2,
        vec![1, 2, 3, 4, 5, 61]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/snapshot/01/02/cpu/2/b"),
        context_hash_2,
        vec![1, 2, 3, 4, 5, 62]
    );
    assert_data_eq!(
        context,
        context_key!("data/rolls/owner/snapshot/01/02/index/123"),
        context_hash_2,
        vec![1, 2, 3, 4, 5, 6, 7]
    );

    Ok(())
}

fn dummy_block(block_hash: &str, level: i32) -> Result<BlockHeaderWithHash, anyhow::Error> {
    Ok(BlockHeaderWithHash {
        hash: block_hash.try_into()?,
        header: Arc::new(
            BlockHeaderBuilder::default()
                .level(level)
                .proto(0)
                .predecessor("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe".try_into()?)
                .timestamp(5_635_634.into())
                .validation_pass(0)
                .operations_hash(
                    "LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc".try_into()?,
                )
                .fitness(Fitness::default())
                .context("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd".try_into()?)
                .protocol_data(vec![].into())
                .build()
                .unwrap(),
        ),
    })
}

#[macro_export]
macro_rules! assert_data_eq {
    ($ctx:expr, $key:expr, $context_hash:expr, $data:expr) => {{
        let data = $ctx.index.get_key_from_history(&$context_hash, &$key)?;
        assert!(data.is_some());
        assert_eq!(data.unwrap(), $data);
    }};
}

#[macro_export]
macro_rules! assert_data_deleted {
    ($ctx:expr, $key:expr, $context_hash:expr) => {{
        let data = $ctx.index.get_key_from_history(&$context_hash, &$key);
        assert!(data.is_ok());
        assert!(data.unwrap().is_none());
    }};
}
