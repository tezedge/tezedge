// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::bail;

use crypto::hash::{BlockHash, ChainId};
use storage::context::ContextApi;
use storage::context::StringTreeEntry;
use storage::{
    context_key, BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader,
    OperationsStorage, OperationsStorageReader,
};
use storage::{BlockAdditionalData, PersistentStorage};
use tezos_messages::p2p::encoding::version::NetworkVersion;

use crate::encoding::chain::BlockInfo;
use crate::helpers::{
    get_context_hash, BlockHeaderInfo, BlockHeaderShellInfo, BlockMetadata, BlockOperation,
    BlockOperations, BlockValidationPass, FullBlockInfo, NodeVersion, Protocols,
};
use crate::server::RpcServiceEnvironment;
use tezos_api::ffi::ApplyBlockRequest;

pub type BlockOperationsHashes = Vec<String>;

/// Retrieve blocks from database.
pub(crate) fn get_block_hashes(
    _chain_id: ChainId,
    block_hash: BlockHash,
    every_nth_level: Option<i32>,
    limit: usize,
    persistent_storage: &PersistentStorage,
) -> Result<Vec<BlockHash>, failure::Error> {
    Ok(match every_nth_level {
        Some(every_nth_level) => {
            BlockStorage::new(persistent_storage).get_every_nth(every_nth_level, &block_hash, limit)
        }
        None => BlockStorage::new(persistent_storage).get_multiple_without_json(&block_hash, limit),
    }?
    .into_iter()
    .map(|block_header| block_header.hash)
    .collect::<Vec<BlockHash>>())
}

/// Get block metadata
pub(crate) fn get_block_metadata(
    _: &ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Option<BlockMetadata>, failure::Error> {
    BlockStorage::new(env.persistent_storage())
        .get_with_json_data(&block_hash)?
        .map(|(block_header, block_json_data)| {
            if let Some(block_additional_data) =
                BlockMetaStorage::new(env.persistent_storage()).get_additional_data(&block_hash)?
            {
                let response = env
                    .tezos_readonly_api()
                    .pool
                    .get()?
                    .api
                    .apply_block_result_metadata(
                        block_header.header.context().clone(),
                        block_json_data.block_header_proto_metadata_bytes,
                        block_additional_data.max_operations_ttl().into(),
                        block_additional_data.protocol_hash,
                        block_additional_data.next_protocol_hash,
                    )?;

                Ok(serde_json::from_str(&response)?)
            } else {
                bail!(
                    "No additional data found for block_hash: {}",
                    block_hash.to_base58_check()
                )
            }
        })
        .transpose()
}

/// Get information about block header
pub(crate) fn get_block_header(
    chain_id: ChainId,
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Option<BlockHeaderInfo>, failure::Error> {
    BlockStorage::new(persistent_storage)
        .get_with_json_data(&block_hash)?
        .map(|(header, json_data)| {
            if let Some(block_additional_data) =
                BlockMetaStorage::new(persistent_storage).get_additional_data(&block_hash)?
            {
                Ok(BlockHeaderInfo::new(
                    &header,
                    &json_data,
                    &block_additional_data,
                    &chain_id,
                ))
            } else {
                bail!(
                    "No additional data found for block_hash: {}",
                    block_hash.to_base58_check()
                )
            }
        })
        .transpose()
}

/// Get information about block shell header
pub(crate) fn get_block_shell_header(
    _: ChainId,
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Option<BlockHeaderShellInfo>, failure::Error> {
    Ok(BlockStorage::new(persistent_storage)
        .get(&block_hash)?
        .map(|header| BlockHeaderShellInfo::new(&header)))
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
    _: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Protocols, failure::Error> {
    if let Some(block_additional_data) =
        BlockMetaStorage::new(persistent_storage).get_additional_data(block_hash)?
    {
        Ok(Protocols::new(
            block_additional_data.protocol_hash().to_base58_check(),
            block_additional_data.next_protocol_hash().to_base58_check(),
        ))
    } else {
        bail!(
            "Cannot retrieve protocols, block_hash {} not found!",
            block_hash.to_base58_check()
        )
    }
}

/// Extract the current_protocol and the next_protocol from the block metadata
pub(crate) fn get_additional_data(
    _: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Option<BlockAdditionalData>, failure::Error> {
    BlockMetaStorage::new(persistent_storage)
        .get_additional_data(&block_hash)
        .map_err(|e| e.into())
}

/// Returns the hashes of all the operations included in the block.
pub(crate) fn get_block_operation_hashes(
    chain_id: ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Vec<BlockOperationsHashes>, failure::Error> {
    let block_operations = get_block_operations_metadata(chain_id, block_hash, env)?;
    let operations = block_operations
        .into_iter()
        .map(|op_group| {
            op_group
                .into_iter()
                .map(|op| op["hash"].to_string().replace("\"", ""))
                .collect()
        })
        .collect();
    Ok(operations)
}

/// Extract all the operations included in the block.
pub(crate) fn get_block_operations_metadata(
    chain_id: ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<BlockOperations, failure::Error> {
    let operations = match BlockStorage::new(env.persistent_storage()).get_json_data(&block_hash)? {
        Some(block_json_data) => {
            if let Some(block_additional_data) =
                BlockMetaStorage::new(env.persistent_storage()).get_additional_data(&block_hash)?
            {
                let operations =
                    OperationsStorage::new(env.persistent_storage()).get_operations(&block_hash)?;

                let response = env
                    .tezos_readonly_api()
                    .pool
                    .get()?
                    .api
                    .apply_block_operations_metadata(
                        chain_id,
                        ApplyBlockRequest::convert_operations(operations),
                        block_json_data.operations_proto_metadata_bytes,
                        block_additional_data.protocol_hash,
                        block_additional_data.next_protocol_hash,
                    )?;

                serde_json::from_str::<BlockOperations>(&response)?
            } else {
                bail!(
                    "No additional data found for block_hash: {}",
                    block_hash.to_base58_check()
                )
            }
        }
        None => {
            bail!(
                "No json data found for block_hash: {}",
                block_hash.to_base58_check()
            )
        }
    };

    Ok(operations)
}

/// Extract all the operations included in the provided validation pass.
pub(crate) fn get_block_operations_validation_pass(
    chain_id: ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
    validation_pass: usize,
) -> Result<BlockValidationPass, failure::Error> {
    let block_operations = get_block_operations_metadata(chain_id, &block_hash, env)?;
    if let Some(block_validation_pass) = block_operations.get(validation_pass) {
        Ok(block_validation_pass.clone())
    } else {
        bail!(
            "Cannot retrieve validation pass {} from block {}",
            validation_pass,
            block_hash.to_base58_check()
        )
    }
}

/// Extract a specific operation included in one of the block's validation pass.
pub(crate) fn get_block_operation(
    chain_id: ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
    validation_pass: usize,
    operation_index: usize,
) -> Result<BlockOperation, failure::Error> {
    let block_operations = get_block_operations_metadata(chain_id, &block_hash, env)?;
    if let Some(block_validation_pass) = block_operations.get(validation_pass) {
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
}

pub(crate) fn get_node_version(network_version: &NetworkVersion) -> NodeVersion {
    NodeVersion::new(network_version)
}

pub(crate) fn get_block(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Option<BlockInfo>, failure::Error> {
    Ok(BlockStorage::new(persistent_storage)
        .get_with_json_data(&block_hash)?
        .map(|(header, json_data)| FullBlockInfo::new(&header, &json_data, chain_id)))
}
