// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::bail;

use crypto::hash::{BlockHash, ChainId};
use storage::block_storage::BlockJsonData;
use storage::context::ContextApi;
use storage::context::StringTreeEntry;
use storage::PersistentStorage;
use storage::{
    context_key, BlockHeaderWithHash, BlockMetaStorage, BlockMetaStorageReader, BlockStorage,
    BlockStorageReader,
};
use tezos_messages::p2p::encoding::version::NetworkVersion;

use crate::helpers::{
    get_context_hash, BlockHeaderInfo, BlockHeaderShellInfo, BlockMetadata, BlockOperation,
    BlockOperations, BlockValidationPass, FullBlockInfo, NodeVersion, Protocols,
};
use crate::server::RpcServiceEnvironment;

pub type BlockOperationsHashes = Vec<String>;

/// Retrieve blocks from database.
pub(crate) fn get_blocks<T>(
    _chain_id: ChainId,
    block_hash: BlockHash,
    every_nth_level: Option<i32>,
    limit: usize,
    persistent_storage: &PersistentStorage,
) -> Result<Vec<T>, failure::Error>
where
    T: From<(BlockHeaderWithHash, BlockJsonData)>,
{
    let block_storage = BlockStorage::new(persistent_storage);
    let blocks = match every_nth_level {
        Some(every_nth_level) => {
            block_storage.get_every_nth_with_json_data(every_nth_level, &block_hash, limit)
        }
        None => block_storage.get_multiple_with_json_data(&block_hash, limit),
    }?
    .into_iter()
    .map(|raw_data| raw_data.into())
    .collect::<Vec<T>>();
    Ok(blocks)
}

/// Get block metadata
pub(crate) fn get_block_metadata(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Option<BlockMetadata>, failure::Error> {
    get_block(chain_id, block_hash, env.persistent_storage()).map(|block| block.map(|b| b.metadata))
}

/// Get information about block header
pub(crate) fn get_block_header(
    chain_id: ChainId,
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Option<BlockHeaderInfo>, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    let block = block_storage
        .get_with_json_data(&block_hash)?
        .map(|(header, json_data)| {
            map_header_and_json_to_block_header_info(header, json_data, &chain_id)
        });

    Ok(block)
}

/// Get information about block shell header
pub(crate) fn get_block_shell_header(
    chain_id: ChainId,
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Option<BlockHeaderShellInfo>, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    let block = block_storage
        .get_with_json_data(&block_hash)?
        .map(|(header, json_data)| {
            map_header_and_json_to_block_header_info(header, json_data, &chain_id).to_shell_header()
        });

    Ok(block)
}

pub(crate) fn live_blocks(
    _: ChainId,
    block_hash: BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Vec<String>, failure::Error> {
    let persistent_storage = env.persistent_storage();

    let block_meta_storage = BlockMetaStorage::new(persistent_storage);

    // get max_ttl for requested block
    let max_ttl: usize = match block_meta_storage.get_additional_data(&block_hash)? {
        Some(additional_data) => additional_data.max_operations_ttl().into(),
        None => bail!(
            "Max_ttl not found for block id: {}",
            block_hash.to_base58_check()
        ),
    };

    // get live blocks
    let live_blocks = block_meta_storage
        .get_live_blocks(block_hash, max_ttl)?
        .iter()
        .map(|block| block.to_base58_check())
        .collect();

    Ok(live_blocks)
}

pub(crate) fn get_context_raw_bytes(
    block_hash: &BlockHash,
    prefix: Option<&str>,
    depth: Option<usize>,
    env: &RpcServiceEnvironment,
) -> Result<StringTreeEntry, failure::Error> {
    // we assume that root is at "/data"
    let mut key_prefix = context_key!("data");

    // clients may pass in a prefix (without /data) with elements containing slashes (expecting us to split)
    // we need to join with '/' and split again
    if let Some(prefix) = prefix {
        key_prefix.extend(prefix.split('/').map(|s| s.to_string()));
    };

    let ctx_hash = get_context_hash(block_hash, env)?;
    Ok(env
        .tezedge_context()
        .get_context_tree_by_prefix(&ctx_hash, &key_prefix, depth)?)
}

/// Extract the current_protocol and the next_protocol from the block metadata
pub(crate) fn get_block_protocols(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Protocols, failure::Error> {
    if let Some(block_info) = get_block(chain_id, &block_hash, persistent_storage)? {
        Ok(Protocols::new(
            block_info.metadata["protocol"]
                .to_string()
                .replace("\"", ""),
            block_info.metadata["next_protocol"]
                .to_string()
                .replace("\"", ""),
        ))
    } else {
        bail!(
            "Cannot retrieve protocols, block_hash {} not found!",
            block_hash.to_base58_check()
        )
    }
}

/// Returns the hashes of all the operations included in the block.
pub(crate) fn get_block_operation_hashes(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Vec<BlockOperationsHashes>, failure::Error> {
    if let Some(block_info) = get_block(chain_id, block_hash, persistent_storage)? {
        let operations = block_info
            .operations
            .into_iter()
            .map(|op_group| {
                op_group
                    .into_iter()
                    .map(|op| op["hash"].to_string().replace("\"", ""))
                    .collect()
            })
            .collect();
        Ok(operations)
    } else {
        bail!(
            "Cannot retrieve operation hashes from block, block_hash {} not found!",
            block_hash.to_base58_check()
        )
    }
}

/// Extract all the operations included in the block.
pub(crate) fn get_block_operations(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<BlockOperations, failure::Error> {
    if let Some(block_info) = get_block(chain_id, &block_hash, persistent_storage)? {
        Ok(block_info.operations)
    } else {
        bail!(
            "Cannot retrieve operations, block_hash {} not found!",
            block_hash.to_base58_check()
        )
    }
}

/// Extract all the operations included in the provided validation pass.
pub(crate) fn get_block_operations_validation_pass(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
    validation_pass: usize,
) -> Result<BlockValidationPass, failure::Error> {
    if let Some(block_info) = get_block(chain_id, &block_hash, persistent_storage)? {
        if let Some(block_validation_pass) = block_info.operations.get(validation_pass) {
            Ok(block_validation_pass.clone())
        } else {
            bail!(
                "Cannot retrieve validation pass {} from block {}",
                validation_pass,
                block_hash.to_base58_check()
            )
        }
    } else {
        bail!(
            "Cannot retrieve operations, block_hash {} not found!",
            block_hash.to_base58_check()
        )
    }
}

/// Extract a specific operation included in one of the block's validation pass.
pub(crate) fn get_block_operation(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
    validation_pass: usize,
    operation_index: usize,
) -> Result<BlockOperation, failure::Error> {
    if let Some(block_info) = get_block(chain_id, &block_hash, persistent_storage)? {
        if let Some(block_validation_pass) = block_info.operations.get(validation_pass) {
            if let Some(operation) = block_validation_pass.get(operation_index) {
                Ok(operation.clone())
            } else {
                bail!(
                    "Cannot retrieve operation {} from validation pass {} from block {}",
                    operation_index,
                    validation_pass,
                    block_hash.to_base58_check()
                )
            }
        } else {
            bail!(
                "Cannot retrieve validation pass {} from block {}",
                validation_pass,
                block_hash.to_base58_check()
            )
        }
    } else {
        bail!(
            "Cannot retrieve operation block, block_hash {} not found!",
            block_hash.to_base58_check()
        )
    }
}

pub(crate) fn get_node_version(network_version: &NetworkVersion) -> NodeVersion {
    NodeVersion::new(network_version)
}

pub(crate) fn get_block(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Option<FullBlockInfo>, failure::Error> {
    Ok(BlockStorage::new(persistent_storage)
        .get_with_json_data(&block_hash)?
        .map(|(header, json_data)| {
            map_header_and_json_to_full_block_info(header, json_data, &chain_id)
        }))
}

#[inline]
fn map_header_and_json_to_full_block_info(
    header: BlockHeaderWithHash,
    json_data: BlockJsonData,
    chain_id: &ChainId,
) -> FullBlockInfo {
    FullBlockInfo::new(&header, &json_data, chain_id)
}

#[inline]
fn map_header_and_json_to_block_header_info(
    header: BlockHeaderWithHash,
    json_data: BlockJsonData,
    chain_id: &ChainId,
) -> BlockHeaderInfo {
    BlockHeaderInfo::new(&header, &json_data, chain_id)
}
