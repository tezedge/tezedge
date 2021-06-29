// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::bail;

use crypto::hash::{BlockHash, ChainId, ContextHash};
use storage::{BlockAdditionalData, PersistentStorage};
use storage::{
    BlockJsonData, BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader,
    OperationsStorage, OperationsStorageReader,
};
use tezos_messages::p2p::encoding::version::NetworkVersion;
use tezos_new_context::{context_key_owned, StringTreeEntry};

use crate::helpers::{
    get_context_hash, BlockHeaderInfo, BlockHeaderShellInfo, BlockInfo, BlockMetadata,
    BlockOperation, BlockOperations, BlockValidationPass, InnerBlockHeader, NodeVersion, Protocols,
};
use crate::server::RpcServiceEnvironment;
use tezos_api::ffi::ApplyBlockRequest;
use tezos_messages::p2p::encoding::prelude::OperationsForBlocksMessage;
use tezos_messages::ts_to_rfc3339;

pub type BlockOperationsHashes = Vec<String>;

use cached::proc_macro::cached;
use cached::SizedCache;
use cached::TimedCache;
use cached::TimedSizedCache;

pub const TIMED_SIZED_CACHE_SIZE: usize = 10;
pub const TIMED_SIZED_CACHE_TTL_IN_SECS: u64 = 20;

/// Retrieve blocks from database.
#[cached(
    name = "BLOCK_HASH_CACHE",
    type = "TimedCache<(ChainId, BlockHash, Option<i32>, usize), Vec<BlockHash>>",
    create = "{TimedCache::with_lifespan(TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(_chain_id.clone(), block_hash.clone(), every_nth_level, limit)}",
    result = true
)]
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
#[cached(
    name = "BLOCK_METADATA_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), BlockMetadata>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(_chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) async fn get_block_metadata(
    _chain_id: &ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<BlockMetadata, failure::Error> {
    // header + jsons
    let block_header_with_json_data = async {
        match BlockStorage::new(env.persistent_storage()).get_with_json_data(block_hash)? {
            Some(data) => Ok(data),
            None => bail!(
                "No block header data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }
    };

    // additional data
    let block_additional_data = async {
        match BlockMetaStorage::new(env.persistent_storage()).get_additional_data(block_hash)? {
            Some(data) => Ok(data),
            None => bail!(
                "No block additional data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }
    };

    // 1. wait for data to collect
    let ((block_header, block_json_data), block_additional_data) =
        futures::try_join!(block_header_with_json_data, block_additional_data,)?;

    convert_block_metadata(
        block_header.header.context().clone(),
        block_json_data.block_header_proto_metadata_bytes,
        &block_additional_data,
        env,
    )
}

fn convert_block_metadata(
    context_hash: ContextHash,
    block_header_proto_metadata_bytes: Vec<u8>,
    block_additional_data: &BlockAdditionalData,
    env: &RpcServiceEnvironment,
) -> Result<BlockMetadata, failure::Error> {
    // TODO: TE-521 - rewrite encoding part to rust
    let response = env
        .tezos_readonly_api()
        .pool
        .get()?
        .api
        .apply_block_result_metadata(
            context_hash,
            block_header_proto_metadata_bytes,
            block_additional_data.max_operations_ttl().into(),
            block_additional_data.protocol_hash.clone(),
            block_additional_data.next_protocol_hash.clone(),
        )?;

    serde_json::from_str::<BlockMetadata>(&response).map_err(|e| e.into())
}

/// Get information about block header
#[cached(
    name = "BLOCK_HEADER_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), BlockHeaderInfo>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) async fn get_block_header(
    chain_id: ChainId,
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<BlockHeaderInfo, failure::Error> {
    // header + jsons
    let block_header_with_json_data = async {
        match BlockStorage::new(persistent_storage).get_with_json_data(&block_hash)? {
            Some(data) => Ok(data),
            None => bail!(
                "No block header data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }
    };

    // additional data
    let block_additional_data = async {
        match BlockMetaStorage::new(persistent_storage).get_additional_data(&block_hash)? {
            Some(data) => Ok(data),
            None => bail!(
                "No block additional data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }
    };

    // 1. wait for data to collect
    let ((block_header, block_json_data), block_additional_data) =
        futures::try_join!(block_header_with_json_data, block_additional_data,)?;

    Ok(BlockHeaderInfo::new(
        &block_header,
        &block_json_data,
        &block_additional_data,
        &chain_id,
    ))
}

/// Get information about block shell header
#[cached(
    name = "BLOCK_SHELL_HEADER_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Option<BlockHeaderShellInfo>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(_chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) fn get_block_shell_header(
    _chain_id: ChainId,
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Option<BlockHeaderShellInfo>, failure::Error> {
    Ok(BlockStorage::new(persistent_storage)
        .get(&block_hash)?
        .map(|header| BlockHeaderShellInfo::new(&header)))
}

#[cached(
    name = "LIVE_BLOCKS_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Vec<String>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(_chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) fn live_blocks(
    _chain_id: ChainId,
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

#[cached(
    name = "CONTEXT_RAW_BYTES_CACHE",
    type = "TimedSizedCache<(BlockHash, Option<String>, Option<usize>), StringTreeEntry>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(block_hash.clone(), prefix.clone(), depth.clone())}",
    result = true
)]
pub(crate) fn get_context_raw_bytes(
    block_hash: &BlockHash,
    prefix: Option<String>,
    depth: Option<usize>,
    env: &RpcServiceEnvironment,
) -> Result<StringTreeEntry, failure::Error> {
    // we assume that root is at "/data"
    let mut key_prefix = context_key_owned!("data");

    // clients may pass in a prefix (without /data) with elements containing slashes (expecting us to split)
    // we need to join with '/' and split again
    if let Some(prefix) = prefix {
        key_prefix.extend(prefix.split('/').map(|s| s.to_string()));
    };

    let ctx_hash = get_context_hash(block_hash, env)?;
    Ok(env
        .tezedge_context()
        .get_context_tree_by_prefix(&ctx_hash, key_prefix, depth)?)
}

/// Extract the current_protocol and the next_protocol from the block metadata
#[cached(
    name = "BLOCK_PROTOCOLS_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Protocols>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(_chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) fn get_block_protocols(
    _chain_id: &ChainId,
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
#[cached(
    name = "BLOCK_ADDITIONAL_DATA_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Option<BlockAdditionalData>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(_chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) fn get_additional_data(
    _chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Option<BlockAdditionalData>, failure::Error> {
    BlockMetaStorage::new(persistent_storage)
        .get_additional_data(&block_hash)
        .map_err(|e| e.into())
}

/// Returns the hashes of all the operations included in the block.
#[cached(
    name = "BLOCK_OPERATION_HASHES_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Vec<BlockOperationsHashes>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) async fn get_block_operation_hashes(
    chain_id: ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Vec<BlockOperationsHashes>, failure::Error> {
    let block_operations = get_block_operations_metadata(chain_id, block_hash, env).await?;
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
#[cached(
    name = "BLOCK_OPERATION_METADATA_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), BlockOperations>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) async fn get_block_operations_metadata(
    chain_id: ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<BlockOperations, failure::Error> {
    // header + jsons
    let block_json_data = async {
        match BlockStorage::new(env.persistent_storage()).get_json_data(block_hash)? {
            Some(data) => Ok(data),
            None => bail!(
                "No block header data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }
    };

    // additional data
    let block_additional_data = async {
        match BlockMetaStorage::new(env.persistent_storage()).get_additional_data(block_hash)? {
            Some(data) => Ok(data),
            None => bail!(
                "No block additional data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }
    };

    // operations
    let operations = async {
        OperationsStorage::new(env.persistent_storage())
            .get_operations(block_hash)
            .map_err(failure::Error::from)
    };

    // 1. wait for data to collect
    let (block_json_data, block_additional_data, operations) =
        futures::try_join!(block_json_data, block_additional_data, operations)?;

    convert_block_operations_metadata(
        chain_id,
        block_json_data.operations_proto_metadata_bytes,
        &block_additional_data,
        operations,
        env,
    )
}

fn convert_block_operations_metadata(
    chain_id: ChainId,
    operations_proto_metadata_bytes: Vec<Vec<Vec<u8>>>,
    block_additional_data: &BlockAdditionalData,
    operations: Vec<OperationsForBlocksMessage>,
    env: &RpcServiceEnvironment,
) -> Result<BlockOperations, failure::Error> {
    // TODO: TE-521 - rewrite encoding part to rust
    let response = env
        .tezos_readonly_api()
        .pool
        .get()?
        .api
        .apply_block_operations_metadata(
            chain_id,
            ApplyBlockRequest::convert_operations(operations),
            operations_proto_metadata_bytes,
            block_additional_data.protocol_hash.clone(),
            block_additional_data.next_protocol_hash.clone(),
        )?;

    serde_json::from_str::<BlockOperations>(&response).map_err(|e| e.into())
}

/// Extract all the operations included in the provided validation pass.
#[cached(
    name = "BLOCK_OPERATION_VP_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash, usize), BlockValidationPass>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone(), validation_pass)}",
    result = true
)]
pub(crate) async fn get_block_operations_validation_pass(
    chain_id: ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
    validation_pass: usize,
) -> Result<BlockValidationPass, failure::Error> {
    let block_operations = get_block_operations_metadata(chain_id, &block_hash, env).await?;
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
#[cached(
    name = "BLOCK_OPERATION_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash, usize, usize), BlockOperation>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone(), validation_pass, operation_index)}",
    result = true
)]
pub(crate) async fn get_block_operation(
    chain_id: ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
    validation_pass: usize,
    operation_index: usize,
) -> Result<BlockOperation, failure::Error> {
    let block_operations = get_block_operations_metadata(chain_id, &block_hash, env).await?;
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

#[cached(
    name = "NODE_VERSION_CACHE",
    type = "SizedCache<NetworkVersion, NodeVersion>",
    create = "{SizedCache::with_size(1)}",
    convert = "{network_version.clone()}"
)]
pub(crate) fn get_node_version(network_version: &NetworkVersion) -> NodeVersion {
    NodeVersion::new(network_version)
}

/// This is heavy operations, collects all various block data.
/// Dont use it, it is dedicated just for one RPC
#[cached(
    name = "BLOCK_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), BlockInfo>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) async fn get_block(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<BlockInfo, failure::Error> {
    // header + jsons
    let block_header_with_json_data = async {
        match BlockStorage::new(env.persistent_storage()).get_with_json_data(block_hash)? {
            Some(data) => Ok(data),
            None => bail!(
                "No block header data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }
    };

    // additional data
    let block_additional_data = async {
        match BlockMetaStorage::new(env.persistent_storage()).get_additional_data(block_hash)? {
            Some(data) => Ok(data),
            None => bail!(
                "No block additional data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }
    };

    // operations
    let operations = async {
        OperationsStorage::new(env.persistent_storage())
            .get_operations(block_hash)
            .map_err(failure::Error::from)
    };

    // 1. wait for data to collect
    let ((block_header, block_json_data), block_additional_data, operations) = futures::try_join!(
        block_header_with_json_data,
        block_additional_data,
        operations
    )?;

    // 2. convert all data
    let BlockJsonData {
        block_header_proto_json,
        block_header_proto_metadata_bytes,
        operations_proto_metadata_bytes,
    } = block_json_data;

    let header = InnerBlockHeader {
        level: block_header.header.level(),
        proto: block_header.header.proto(),
        predecessor: block_header.header.predecessor().to_base58_check(),
        timestamp: ts_to_rfc3339(block_header.header.timestamp()),
        validation_pass: block_header.header.validation_pass(),
        operations_hash: block_header.header.operations_hash().to_base58_check(),
        fitness: block_header
            .header
            .fitness()
            .iter()
            .map(|x| hex::encode(&x))
            .collect(),
        context: block_header.header.context().to_base58_check(),
        protocol_data: serde_json::from_str(&block_header_proto_json).unwrap_or_default(),
    };

    // TODO: TE-521 - rewrite encoding part to rust - this two calls could be parallelized (once we have our encodings in rust)
    let metadata = convert_block_metadata(
        block_header.header.context().clone(),
        block_header_proto_metadata_bytes,
        &block_additional_data,
        env,
    )?;
    let block_operations = convert_block_operations_metadata(
        chain_id.clone(),
        operations_proto_metadata_bytes,
        &block_additional_data,
        operations,
        env,
    )?;

    Ok(BlockInfo::new(
        chain_id,
        block_hash,
        block_additional_data.protocol_hash,
        header,
        metadata,
        block_operations,
    ))
}
