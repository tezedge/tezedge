// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Storage snapshots command. Produces a new trimmed copy of the source storage,
//! with all the context history and block application results for blocks before
//! the specified target block removed. If no target block is specified, defaults
//! to HEAD~10.
//!
//! Example:
//!
//! ```
//! ./target/release/light-node \
//!     --config-file ./light_node/etc/tezedge/tezedge.config \
//!     --tezos-data-dir /path/to/source \
//!     snapshot \
//!     --target-path /path/to/target \
//!     --block 434223 # optional block hash, level of negative offset from head
//! ```

use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use tempfile::tempdir_in;

use slog::{info, Logger};

use crypto::hash::{BlockHash, ContextHash};
use storage::{
    initialize_storage_with_genesis_block, store_commit_genesis_result, BlockMetaStorage,
    BlockMetaStorageReader, BlockReference, BlockStorage, BlockStorageReader, ChainMetaStorage,
    ChainMetaStorageReader, ConstantsStorage, CycleErasStorage, CycleMetaStorage,
    OperationsMetaStorage, OperationsStorage, OperationsStorageReader, PersistentStorage,
    StorageInitInfo, SystemStorage,
};

use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use tezos_api::ffi::CommitGenesisResult;
use tezos_messages::Head;
use tezos_protocol_ipc_client::{ProtocolRunnerApi, ProtocolRunnerError};
use tokio::process::Child;

use crate::{
    create_protocol_runner_configuration, create_tokio_runtime, initialize_persistent_storage,
};

pub fn snapshot_storage(
    env: crate::configuration::Environment,
    persistent_storage: PersistentStorage,
    init_storage_data: StorageInitInfo,
    target_block: Option<BlockReference>,
    target_path: PathBuf,
    log: Logger,
) {
    let final_path = target_path;
    let tmpdir = tempdir_in(&final_path)
        .expect("Failed to create temporary path for building the new storage");
    let target_path = tmpdir.path().to_path_buf();

    // We don't try to figure a savepoint way back on history, so if no block is specified,
    // we default to going back just a few blocks based on the information here:
    // https://tezos.stackexchange.com/questions/3539/how-often-do-blockchain-reorgs-happen
    let target_block = target_block.unwrap_or(BlockReference::OffsetFromHead(10));

    info!(log, "Fetching data from source main storage...");

    let system_storage = SystemStorage::new(persistent_storage.main_db());
    let block_storage = BlockStorage::new(&persistent_storage);
    let block_meta_storage = BlockMetaStorage::new(&persistent_storage);
    let operations_storage = OperationsStorage::new(&persistent_storage);
    let chain_meta_storage = ChainMetaStorage::new(&persistent_storage);
    let constants_storage = ConstantsStorage::new(&persistent_storage);
    let cycles_storage = CycleMetaStorage::new(&persistent_storage);
    let cycle_eras_storage = CycleErasStorage::new(&persistent_storage);

    let chain_id = system_storage
        .get_chain_id()
        .expect("Failed to obtain chain id from new storage")
        .expect("Failed to obtain chain id from new storage");

    let head = chain_meta_storage
        .get_current_head(&chain_id)
        .expect("Failed to obtain the current head from the source storage")
        .expect("Source storage does not have a current head");

    let target_block =
        resolve_block_reference(target_block, &block_storage, &block_meta_storage, &head);

    let (block_header_with_hash, block_json_data) = block_storage
        .get_with_json_data(&target_block)
        .unwrap_or_else(|_| {
            panic!(
                "Failed to obtain block data for {}",
                target_block.to_base58_check()
            )
        })
        .unwrap_or_else(|| {
            panic!(
                "Failed to obtain block data for {}",
                target_block.to_base58_check()
            )
        });
    let block_additional_data = block_meta_storage
        .get_additional_data(&target_block)
        .unwrap_or_else(|_| {
            panic!(
                "Failed to obtain additional block data for {}",
                target_block.to_base58_check()
            )
        });

    let context_hash = block_header_with_hash.header.context().clone();

    info!(log, "Fetched block"; "block_hash" => target_block.to_base58_check(), "is_applied" => block_meta_storage.is_applied(&target_block).unwrap());

    let constants_data = constants_storage
        .iterator()
        .expect("Failed to obtain constants data");
    let cycles_data = cycles_storage
        .iterator()
        .expect("Failed to obtain cycles data");
    let cycle_eras_data = cycle_eras_storage
        .iterator()
        .expect("Failed to obtain cycle eras data");

    let (genesis_commit_hash, genesis_result) =
        initialize_protocol_runner_and_snapshot_context(&env, &context_hash, &target_path, &log);

    // Env for snapshot with modified path
    let mut target_env = env.clone();
    target_env.storage.db_path = target_path.join("bootstrap_db");
    target_env.storage.db.db_path = target_env.storage.db_path.join("db");

    info!(log, "Initializing snapshot main storage...");

    // Init new main storage on target directory and put into it data from the source main storage
    let new_persistent_storage = initialize_persistent_storage(&target_env, &log);

    let new_system_storage = SystemStorage::new(new_persistent_storage.main_db());
    let new_chain_meta_storage = ChainMetaStorage::new(&new_persistent_storage);
    let new_block_storage = BlockStorage::new(&new_persistent_storage);
    let new_block_meta_storage = BlockMetaStorage::new(&new_persistent_storage);
    let new_operations_storage = OperationsStorage::new(&new_persistent_storage);
    let new_operations_meta_storage = OperationsMetaStorage::new(&new_persistent_storage);
    let new_constants_storage = ConstantsStorage::new(&new_persistent_storage);
    let new_cycles_storage = CycleMetaStorage::new(&new_persistent_storage);
    let new_cycle_eras_storage = CycleErasStorage::new(&new_persistent_storage);

    let chain_id = new_system_storage
        .get_chain_id()
        .expect("Failed to obtain chain id from new storage")
        .expect("Failed to obtain chain id from new storage");

    info!(log, "Initializing genesis block data on new storage...");

    let new_genesis_block = initialize_storage_with_genesis_block(
        &new_block_storage,
        &new_block_meta_storage,
        &init_storage_data,
        &env.tezos_network_config,
        &genesis_commit_hash,
        &log,
    )
    .expect("Failed to initialize new main storage with genesis block");

    store_commit_genesis_result(
        &new_block_storage,
        &new_block_meta_storage,
        &new_chain_meta_storage,
        &new_operations_meta_storage,
        &init_storage_data,
        genesis_result,
    )
    .expect("Failed to store genesis commit result to new main storage");

    // Store all block headers up until target_block (included)
    info!(log, "Copying block headers up until the target block...");

    let mut current_block_hash = new_genesis_block.hash.clone();
    let mut block_count = 0;
    let mut operations_count = 0;

    'outer: loop {
        let blocks = block_storage
            .get_multiple_without_json(&current_block_hash, 100)
            .expect("Failed when obtaining block headers data from source sorage");

        for block_header_with_hash in &blocks {
            if block_header_with_hash.hash != new_genesis_block.hash {
                new_block_storage
                    .put_block_header(block_header_with_hash)
                    .expect("Failed to store block header to new main storage");
                block_count += 1;

                let operations_data = operations_storage
                    .get_operations(&block_header_with_hash.hash)
                    .expect("Failed to obtain operations for block");

                for message in operations_data {
                    new_operations_storage
                        .put_operations(&message)
                        .expect("Failed to store operations data into new main storage");
                    operations_count += 1;
                }

                let block_meta = new_block_meta_storage
                    .put_block_header_with_applied(&block_header_with_hash, &chain_id, &log)
                    .expect("Failed to store block header meta to new main storage");
                new_block_meta_storage
                    .store_predecessors(&block_header_with_hash.hash, &block_meta)
                    .expect("Failed to store predecessors metadata to new main storage");
            }

            // Last block was the target block, skip the rest
            if block_header_with_hash.hash == target_block {
                break 'outer;
            }
        }

        current_block_hash = blocks
            .last()
            .expect("Reached end of chain of blocks without finding the target block hash")
            .hash
            .clone();
    }

    info!(log, "Done copying block headers and operations"; "block_count" => block_count, "operations_count" => operations_count);

    info!(log, "Storing result data for target block...");

    new_block_storage
        .put_block_json_data(&block_header_with_hash.hash, block_json_data)
        .expect("Failed to store block json data to new main storage");
    if let Some(additional_data) = &block_additional_data {
        new_block_meta_storage
            .put_block_additional_data(&block_header_with_hash.hash, additional_data)
            .expect("Failed to store block additional data to new main storage");
    }

    info!(log, "Set head result"; "block_hash" => block_header_with_hash.hash.to_base58_check(), "is_applied" => new_block_meta_storage.is_applied(&target_block).unwrap());

    // Set current head on new storage
    let head = Head::new(
        block_header_with_hash.hash.clone(),
        block_header_with_hash.header.level(),
        block_header_with_hash.header.fitness().clone(),
    );
    new_chain_meta_storage
        .set_current_head(&chain_id, head.clone())
        .expect("Failed to set current head to new main storage");
    new_chain_meta_storage
        .set_caboose(&chain_id, head)
        .expect("Failed to set caboose to new main storage");

    for (protocol_hash, constants) in constants_data {
        new_constants_storage
            .store_constants_data(protocol_hash, constants)
            .expect("Failed to store protocol constants data to new main storage");
    }

    for (cycle, cycle_data) in cycles_data {
        new_cycles_storage
            .put(&cycle, &cycle_data)
            .expect("Failed to store cycles data to new main storage");
    }

    for (cycle, cycle_era_data) in cycle_eras_data {
        new_cycle_eras_storage
            .put(&cycle, cycle_era_data)
            .expect("Failed to store cycle eras data to new main storage");
    }

    let head = new_chain_meta_storage.get_current_head(&chain_id).unwrap();
    info!(log, "Stored current head = {:?}", head);

    // Move temporary data to final target path
    for entry in target_path
        .read_dir()
        .expect("read_dir call failed")
        .flatten()
    {
        let source_name = entry.file_name();
        let to_path = final_path.join(&source_name);

        info!(log, "Moving storage data into final location"; "source_name" => format!("{}", source_name.to_string_lossy()), "to_path" => format!("{}", to_path.to_string_lossy()));

        std::fs::rename(entry.path(), to_path).expect("Failed to move storage into final location");
    }
}

async fn terminate_or_kill(process: &mut Child, reason: String) -> Result<(), ProtocolRunnerError> {
    // try to send SIGINT (ctrl-c)
    if let Some(pid) = process.id() {
        let pid = Pid::from_raw(pid as i32);
        match signal::kill(pid, Signal::SIGINT) {
            Ok(_) => Ok(()),
            Err(sigint_error) => {
                // (fallback) if SIGINT failed, we just kill process
                match process.kill().await {
                    Ok(_) => Ok(()),
                    Err(kill_error) => Err(ProtocolRunnerError::TerminateError {
                        reason: format!(
                            "Reason for termination: {}, sigint_error: {}, kill_error: {}",
                            reason, sigint_error, kill_error
                        ),
                    }),
                }
            }
        }
    } else {
        Ok(())
    }
}

fn initialize_protocol_runner_and_snapshot_context(
    env: &crate::configuration::Environment,
    context_hash: &ContextHash,
    target_path: &Path,
    log: &Logger,
) -> (ContextHash, CommitGenesisResult) {
    let tokio_runtime = create_tokio_runtime(env).expect("Failed to create tokio runtime");

    let (_context_init_status_sender, context_init_status_receiver) =
        tokio::sync::watch::channel(false);
    let protocol_runner_configuration = create_protocol_runner_configuration(env);
    let mut tezos_protocol_api = ProtocolRunnerApi::new(
        protocol_runner_configuration,
        context_init_status_receiver,
        tokio_runtime.handle(),
        log.clone(),
    );

    tokio_runtime.block_on(async {
        info!(log, "Initializing protocol runner...");

        let mut child = tezos_protocol_api
            .start(None)
            .await
            .expect("Failed to launch protocol runner");
        let mut conn = tezos_protocol_api.connect().await.expect("Failed to connect to protocol runner");

        let _result = conn
            .init_protocol_for_write(false, &env.storage.patch_context, None)
            .await
            .expect("Failed to initialize protocol runner for write (source context");

        info!(log, "Taking (irmin) context snapshot...");

        let tmpdir = tempdir_in(target_path).expect("Could not create a temporary directory for the context dump");
        let context_dump_path = tmpdir
            .path()
            .join("context-dump")
            .to_string_lossy()
            .to_string();

        // Dump context
        let instant = Instant::now();
        let nb_context_elements = conn
            .dump_context(context_hash.clone(), context_dump_path.clone())
            .await
            .expect("Failed to produce a context dump");
        let dump_time = instant.elapsed();

        // TODO: adjust storage in api instead and re-connect?
        info!(log, "Initializing target context...");
        conn.configuration.storage = conn
            .configuration
            .storage
            .with_path(target_path.to_string_lossy().into());

        let init_context_result = conn
            .init_protocol_for_write(true, &env.storage.patch_context, None)
            .await
            .expect("Failed to initialize protocol runner for write (target context");

        let genesis_commit_hash = init_context_result.genesis_commit_hash.expect("Expected genesis commit hash not found");

        let genesis_result = conn.genesis_result_data(&genesis_commit_hash).await.expect("Failed to obtain genesis commit result data");

        // restore it into target directory
        info!(log, "Restoring context from dump...");
        let instant = Instant::now();
        conn.restore_context(context_hash.clone(), context_dump_path.clone(), nb_context_elements)
            .await
            .expect("Failed to restore new context from dump");
        let restore_time = instant.elapsed();

        info!(
            log,
            "Done dumping context"; "dump_time" => format!("{:?}", dump_time), "restore_time" => format!("{:?}", restore_time)
        );

        terminate_or_kill(&mut child, "Done".into()).await.unwrap();

        (genesis_commit_hash, genesis_result)
    })
}

fn resolve_block_reference(
    block_reference: BlockReference,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    head: &Head,
) -> BlockHash {
    match block_reference {
        BlockReference::BlockHash(block_hash) => block_hash,
        BlockReference::Level(level) => block_storage
            .get_by_level(level as i32)
            .unwrap_or_else(|_| panic!("Failed to obtain block at level {}", level))
            .unwrap_or_else(|| panic!("Failed to obtain block at level {}", level))
            .hash
            .clone(),
        BlockReference::OffsetFromHead(offset) => block_meta_storage
            .find_block_at_distance(head.block_hash().clone(), offset)
            .expect("Failed to obtain predecessor")
            .unwrap_or_else(|| head.block_hash().clone()),
    }
}
