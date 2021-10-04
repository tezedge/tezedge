// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crypto::hash::{BlockHash, ChainId, ContextHash};
use storage::{BlockAdditionalData, BlockHeaderWithHash, PersistentStorage};
use storage::{
    BlockJsonData, BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader,
    OperationsStorage, OperationsStorageReader,
};
use tezos_context_api::{context_key_owned, StringTreeObject};
use tezos_messages::p2p::encoding::version::NetworkVersion;

use crate::helpers::{
    BlockHeaderInfo, BlockHeaderShellInfo, BlockInfo, BlockMetadata, BlockOperation,
    BlockOperations, BlockValidationPass, InnerBlockHeader, NodeVersion, Protocols,
    RpcServiceError,
};
use crate::server::RpcServiceEnvironment;
use tezos_api::ffi::ApplyBlockRequest;
use tezos_messages::p2p::encoding::prelude::OperationsForBlocksMessage;
use tezos_messages::ts_to_rfc3339;

pub type BlockOperationsHashes = Vec<String>;

use cached::proc_macro::cached;
use cached::SizedCache;
use cached::TimedSizedCache;

pub const TIMED_SIZED_CACHE_SIZE: usize = 10;
pub const TIMED_SIZED_CACHE_TTL_IN_SECS: u64 = 60;

/// Retrieve blocks from database.
// TODO: TE-572 - rework cache queries + add test to `src\services\mod -> mod tests`
// #[cached(
//     name = "BLOCK_HASH_CACHE",
//     type = "TimedCache<(ChainId, BlockHash, Option<i32>, usize), Vec<BlockHash>>",
//     create = "{TimedCache::with_lifespan(TIMED_SIZED_CACHE_TTL_IN_SECS)}",
//     convert = "{(_chain_id.clone(), block_hash.clone(), every_nth_level, limit)}",
//     result = true
// )]
// TODO: TE-685
// pub(crate) fn get_block_hashes(
//     _chain_id: ChainId,
//     block_hash: BlockHash,
//     every_nth_level: Option<i32>,
//     limit: usize,
//     persistent_storage: &PersistentStorage,
// ) -> Result<Vec<BlockHash>, RpcServiceError> {
//     Ok(match every_nth_level {
//         Some(every_nth_level) => {
//             BlockStorage::new(persistent_storage).get_every_nth(every_nth_level, &block_hash, limit)
//         }
//         None => BlockStorage::new(persistent_storage).get_multiple_without_json(&block_hash, limit),
//     }?
//     .into_iter()
//     .map(|block_header| block_header.hash)
//     .collect::<Vec<BlockHash>>())
// }

/// Get block metadata
#[cached(
    name = "BLOCK_METADATA_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Arc<BlockMetadata>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) async fn get_block_metadata(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Arc<BlockMetadata>, RpcServiceError> {
    // TODO - TE-709: these two  sync calls, need to be wrapped in `tokio::task::spawn_blocking`

    // header + jsons
    let block_header_with_json_data =
        async { get_block_with_json_data(chain_id, block_hash, env.persistent_storage()) };

    // additional data
    let block_additional_data = async {
        crate::services::base_services::get_additional_data_or_fail(
            chain_id,
            block_hash,
            env.persistent_storage(),
        )
    };

    // 1. wait for data to collect
    let (block_header_with_json_data, block_additional_data) =
        tokio::try_join!(block_header_with_json_data, block_additional_data,)?;

    let block_header = &block_header_with_json_data.0;
    let block_json_data = &block_header_with_json_data.1;

    convert_block_metadata(
        block_header.header.context().clone(),
        block_json_data.block_header_proto_metadata_bytes.clone(),
        &block_additional_data,
        env,
    )
    .await
    .map(Arc::new)
}

async fn convert_block_metadata(
    context_hash: ContextHash,
    block_header_proto_metadata_bytes: Vec<u8>,
    block_additional_data: &BlockAdditionalData,
    env: &RpcServiceEnvironment,
) -> Result<BlockMetadata, RpcServiceError> {
    // TODO: TE-521 - rewrite encoding part to rust
    let response = env
        .tezos_protocol_api()
        .readable_connection()
        .await?
        .apply_block_result_metadata(
            context_hash,
            block_header_proto_metadata_bytes,
            block_additional_data.max_operations_ttl().into(),
            block_additional_data.protocol_hash.clone(),
            block_additional_data.next_protocol_hash.clone(),
        )
        .await
        .map_err(|e| RpcServiceError::UnexpectedError {
            reason: format!("Failed to call ffi, reason: {}", e),
        })?;

    serde_json::from_str::<BlockMetadata>(&response).map_err(|e| e.into())
}

/// Get information about block header
#[cached(
    name = "BLOCK_HEADER_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Arc<BlockHeaderInfo>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) async fn get_block_header(
    chain_id: ChainId,
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Arc<BlockHeaderInfo>, RpcServiceError> {
    // TODO - TE-709: these two  sync calls, need to be wrapped in `tokio::task::spawn_blocking`

    // header + jsons
    let block_header_with_json_data =
        async { get_block_with_json_data(&chain_id, &block_hash, persistent_storage) };

    // additional data
    let block_additional_data = async {
        crate::services::base_services::get_additional_data_or_fail(
            &chain_id,
            &block_hash,
            persistent_storage,
        )
    };

    // 1. wait for data to collect
    let (block_header_with_json_data, block_additional_data) =
        tokio::try_join!(block_header_with_json_data, block_additional_data,)?;

    let block_header = &block_header_with_json_data.0;
    let block_json_data = &block_header_with_json_data.1;

    Ok(Arc::new(BlockHeaderInfo::try_new(
        &block_header,
        &block_json_data,
        &block_additional_data,
        &chain_id,
    )?))
}

/// Get information about block shell header
#[cached(
    name = "BLOCK_SHELL_HEADER_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Arc<BlockHeaderShellInfo>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) fn get_block_shell_header_or_fail(
    chain_id: &ChainId,
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Arc<BlockHeaderShellInfo>, RpcServiceError> {
    get_raw_block_header_with_hash(chain_id, &block_hash, persistent_storage)
        .and_then(|block_header| {
            BlockHeaderShellInfo::try_new(&block_header).map_err(RpcServiceError::from)
        })
        .map(|block_header| Arc::new(block_header))
}

#[cached(
    name = "LIVE_BLOCKS_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Vec<String>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) fn live_blocks(
    chain_id: &ChainId,
    block_hash: BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Vec<String>, RpcServiceError> {
    let persistent_storage = env.persistent_storage();

    // get max_ttl for requested block
    let max_ttl: usize = crate::services::base_services::get_additional_data_or_fail(
        &chain_id,
        &block_hash,
        env.persistent_storage(),
    )?
    .max_operations_ttl()
    .into();

    let block_meta_storage = BlockMetaStorage::new(persistent_storage);

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
    type = "TimedSizedCache<(ChainId, BlockHash, Option<String>, Option<usize>), Arc<StringTreeObject>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone(), prefix.clone(), depth.clone())}",
    result = true
)]
pub(crate) async fn get_context_raw_bytes(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    prefix: Option<String>,
    depth: Option<usize>,
    env: &RpcServiceEnvironment,
) -> Result<Arc<StringTreeObject>, RpcServiceError> {
    // we assume that root is at "/data"
    let mut key_prefix = context_key_owned!("data");

    // clients may pass in a prefix (without /data) with elements containing slashes (expecting us to split)
    // we need to join with '/' and split again
    if let Some(prefix) = prefix {
        key_prefix.extend(prefix.split('/').map(|s| s.to_string()));
    };

    let ctx_hash = get_context_hash(chain_id, block_hash, env)?;
    Ok(Arc::new(
        env.tezedge_context()
            .get_context_tree_by_prefix(&ctx_hash, key_prefix, depth)
            .await
            .map_err(|e| RpcServiceError::UnexpectedError {
                reason: format!("{}", e),
            })?,
    ))
}

/// Extract the current_protocol and the next_protocol from the block metadata
#[cached(
    name = "BLOCK_PROTOCOLS_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Protocols>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) fn get_block_protocols(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Protocols, RpcServiceError> {
    let block_additional_data = crate::services::base_services::get_additional_data_or_fail(
        chain_id,
        block_hash,
        persistent_storage,
    )?;
    Ok(Protocols::new(
        block_additional_data.protocol_hash().to_base58_check(),
        block_additional_data.next_protocol_hash().to_base58_check(),
    ))
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
) -> Result<Vec<BlockOperationsHashes>, RpcServiceError> {
    let block_operations = get_block_operations_metadata(chain_id, block_hash, env).await?;
    let operations = block_operations
        .iter()
        .map(|op_group| {
            op_group
                .iter()
                .map(|op| op["hash"].to_string().replace("\"", ""))
                .collect()
        })
        .collect();
    Ok(operations)
}

/// Extract all the operations included in the block.
#[cached(
    name = "BLOCK_OPERATION_METADATA_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Arc<BlockOperations>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) async fn get_block_operations_metadata(
    chain_id: ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Arc<BlockOperations>, RpcServiceError> {
    // TODO - TE-709: these two  sync calls, need to be wrapped in `tokio::task::spawn_blocking`

    // header + jsons
    let block_json_data = async {
        match BlockStorage::new(env.persistent_storage()).get_json_data(block_hash) {
            Ok(Some(data)) => Ok(data),
            Ok(None) => Err(RpcServiceError::NoDataFoundError {
                reason: format!(
                    "No block header data found for block_hash: {}",
                    block_hash.to_base58_check()
                ),
            }),
            Err(e) => Err(RpcServiceError::StorageError { error: e }),
        }
    };

    // additional data
    let block_additional_data = async {
        crate::services::base_services::get_additional_data_or_fail(
            &chain_id,
            block_hash,
            env.persistent_storage(),
        )
    };

    // operations
    let operations = async {
        OperationsStorage::new(env.persistent_storage())
            .get_operations(block_hash)
            .map_err(|error| RpcServiceError::StorageError { error })
    };

    // 1. wait for data to collect
    let (block_json_data, block_additional_data, operations) =
        tokio::try_join!(block_json_data, block_additional_data, operations)?;

    convert_block_operations_metadata(
        chain_id,
        block_json_data.operations_proto_metadata_bytes,
        &block_additional_data,
        operations,
        env,
    )
    .await
    .map(Arc::new)
}

async fn convert_block_operations_metadata(
    chain_id: ChainId,
    operations_proto_metadata_bytes: Vec<Vec<Vec<u8>>>,
    block_additional_data: &BlockAdditionalData,
    operations: Vec<OperationsForBlocksMessage>,
    env: &RpcServiceEnvironment,
) -> Result<BlockOperations, RpcServiceError> {
    // TODO: TE-521 - rewrite encoding part to rust
    let response = env
        .tezos_protocol_api()
        .readable_connection()
        .await?
        .apply_block_operations_metadata(
            chain_id,
            ApplyBlockRequest::convert_operations(operations),
            operations_proto_metadata_bytes,
            block_additional_data.protocol_hash.clone(),
            block_additional_data.next_protocol_hash.clone(),
        )
        .await
        .map_err(|e| RpcServiceError::UnexpectedError {
            reason: format!("Failed to call ffi, reason: {}", e),
        })?;

    serde_json::from_str::<BlockOperations>(&response).map_err(|e| e.into())
}

/// Extract all the operations included in the provided validation pass.
#[cached(
    name = "BLOCK_OPERATION_VP_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash, usize), Arc<BlockValidationPass>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone(), validation_pass)}",
    result = true
)]
pub(crate) async fn get_block_operations_validation_pass(
    chain_id: ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
    validation_pass: usize,
) -> Result<Arc<BlockValidationPass>, RpcServiceError> {
    let block_operations = get_block_operations_metadata(chain_id, &block_hash, env).await?;
    if let Some(block_validation_pass) = block_operations.get(validation_pass) {
        Ok(Arc::new(block_validation_pass.clone()))
    } else {
        Err(RpcServiceError::UnexpectedError {
            reason: format!(
                "Cannot retrieve validation pass {} from block {}",
                validation_pass,
                block_hash.to_base58_check()
            ),
        })
    }
}

/// Extract a specific operation included in one of the block's validation pass.
#[cached(
    name = "BLOCK_OPERATION_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash, usize, usize), Arc<BlockOperation>>",
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
) -> Result<Arc<BlockOperation>, RpcServiceError> {
    let block_operations = get_block_operations_metadata(chain_id, &block_hash, env).await?;
    if let Some(block_validation_pass) = block_operations.get(validation_pass) {
        if let Some(operation) = block_validation_pass.get(operation_index) {
            Ok(Arc::new(operation.clone()))
        } else {
            Err(RpcServiceError::UnexpectedError {
                reason: format!(
                    "Cannot retrieve operation {} from validation pass {} from block {}",
                    operation_index,
                    validation_pass,
                    block_hash.to_base58_check()
                ),
            })
        }
    } else {
        Err(RpcServiceError::UnexpectedError {
            reason: format!(
                "Cannot retrieve validation pass {} from block {}",
                validation_pass,
                block_hash.to_base58_check()
            ),
        })
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
    type = "TimedSizedCache<(ChainId, BlockHash), Arc<BlockInfo>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) async fn get_block(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Arc<BlockInfo>, RpcServiceError> {
    // TODO - TE-709: these two  sync calls, need to be wrapped in `tokio::task::spawn_blocking`

    // header + jsons
    let block_header_with_json_data =
        async { get_block_with_json_data(chain_id, block_hash, env.persistent_storage()) };

    // additional data
    let block_additional_data = async {
        crate::services::base_services::get_additional_data_or_fail(
            chain_id,
            block_hash,
            env.persistent_storage(),
        )
    };

    // operations
    let operations = async {
        OperationsStorage::new(env.persistent_storage())
            .get_operations(block_hash)
            .map_err(|error| RpcServiceError::StorageError { error })
    };

    // 1. wait for data to collect
    let (block_header_with_json_data, block_additional_data, operations) = tokio::try_join!(
        block_header_with_json_data,
        block_additional_data,
        operations
    )?;

    let block_json_data = &block_header_with_json_data.1;
    let block_header = &block_header_with_json_data.0;

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
        timestamp: ts_to_rfc3339(block_header.header.timestamp())?,
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
        block_header_proto_metadata_bytes.clone(),
        &block_additional_data,
        env,
    )
    .await?;
    let block_operations = convert_block_operations_metadata(
        chain_id.clone(),
        operations_proto_metadata_bytes.clone(),
        &block_additional_data,
        operations,
        env,
    )
    .await?;

    Ok(Arc::new(BlockInfo::new(
        chain_id,
        block_hash,
        block_additional_data.protocol_hash.clone(),
        header,
        metadata,
        block_operations,
    )))
}

/// TODO: TE-238 - optimize context_hash/level index, not do deserialize whole header
/// TODO: returns context_hash and level, but level is here just for one use-case, so maybe it could be splitted
pub(crate) fn get_context_hash(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<ContextHash, RpcServiceError> {
    get_raw_block_header_with_hash(chain_id, block_hash, env.persistent_storage())
        .map(|block_header| block_header.header.context().clone())
}

/// Cached database call for additional block data
#[cached(
    name = "BLOCK_ADDITIONAL_DATA_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Arc<BlockAdditionalData>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(_chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) fn get_additional_data_or_fail(
    _chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Arc<BlockAdditionalData>, RpcServiceError> {
    match BlockMetaStorage::new(persistent_storage).get_additional_data(&block_hash) {
        Ok(Some(data)) => Ok(Arc::new(data)),
        Ok(None) => Err(RpcServiceError::NoDataFoundError {
            reason: format!(
                "No block additional data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }),
        Err(se) => Err(RpcServiceError::StorageError { error: se }),
    }
}

#[cached(
    name = "BLOCK_RAW_BLOCK_HEADER_DATA_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Arc<BlockHeaderWithHash>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(_chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) fn get_raw_block_header_with_hash(
    _chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Arc<BlockHeaderWithHash>, RpcServiceError> {
    match BlockStorage::new(persistent_storage).get(&block_hash) {
        Ok(Some(data)) => Ok(Arc::new(data)),
        Ok(None) => Err(RpcServiceError::NoDataFoundError {
            reason: format!(
                "No block header data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }),
        Err(se) => Err(RpcServiceError::StorageError { error: se }),
    }
}

/// Cached database call for block header + jsons
#[cached(
    name = "BLOCK_WITH_JSON_DATA_CACHE",
    type = "TimedSizedCache<(ChainId, BlockHash), Arc<(BlockHeaderWithHash, BlockJsonData)>>",
    create = "{TimedSizedCache::with_size_and_lifespan(TIMED_SIZED_CACHE_SIZE, TIMED_SIZED_CACHE_TTL_IN_SECS)}",
    convert = "{(_chain_id.clone(), block_hash.clone())}",
    result = true
)]
pub(crate) fn get_block_with_json_data(
    _chain_id: &ChainId,
    block_hash: &BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Arc<(BlockHeaderWithHash, BlockJsonData)>, RpcServiceError> {
    match BlockStorage::new(persistent_storage).get_with_json_data(&block_hash) {
        Ok(Some(data)) => Ok(Arc::new(data)),
        Ok(None) => Err(RpcServiceError::NoDataFoundError {
            reason: format!(
                "No block header/json data found for block_hash: {}",
                block_hash.to_base58_check()
            ),
        }),
        Err(se) => Err(RpcServiceError::StorageError { error: se }),
    }
}
