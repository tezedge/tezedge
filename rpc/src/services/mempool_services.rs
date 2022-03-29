// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::convert::TryInto;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use shell::validation::CanApplyStatus;
use shell_automaton::service::rpc_service::RpcRequest as RpcShellAutomatonMsg;
use slog::{info, warn};

use crypto::hash::{BlockHash, ChainId, OperationHash};
use storage::block_meta_storage::Meta;
// use shell_integration::*;
use storage::{
    BlockHeaderWithHash, BlockMetaStorage, BlockMetaStorageReader, BlockStorage,
    OperationsMetaStorage, OperationsStorage, StorageError,
};
use tezos_messages::p2p::binary_message::{BinaryRead, MessageHash};
use tezos_messages::p2p::encoding::operation::DecodedOperation;
use tezos_messages::p2p::encoding::operations_for_blocks::{
    OperationsForBlock, OperationsForBlocksMessage,
};
use tezos_messages::p2p::encoding::prelude::Path;
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};

use crate::helpers::RpcServiceError;
use crate::server::RpcServiceEnvironment;

// const INJECT_BLOCK_WAIT_TIMEOUT: Duration = Duration::from_secs(60);
const INJECT_OPERATION_WAIT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct InjectedBlockWithOperations {
    pub data: String,
    pub operations: Vec<Vec<DecodedOperation>>,
}

pub async fn get_pending_operations(
    _chain_id: &ChainId,
    env: Arc<RpcServiceEnvironment>,
) -> Result<serde_json::Value, RpcServiceError> {
    env.shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetPendingOperations)
        .await
        .map_err(|_| RpcServiceError::UnexpectedError {
            reason: "the channel between rpc and shell is overflown".to_string(),
        })?
        .await
        .map_err(|_| RpcServiceError::UnexpectedError {
            reason: "state machine failed to respond".to_string(),
        })
}

pub async fn inject_operation(
    is_async: bool,
    chain_id: ChainId,
    operation_data: &str,
    env: Arc<RpcServiceEnvironment>,
) -> Result<String, RpcServiceError> {
    info!(env.log(),
          "Operation injection requested";
          "chain_id" => chain_id.to_base58_check(),
          "is_async" => is_async,
          "operation_data" => operation_data,
    );

    let start_request = Instant::now();

    // parse operation data
    let operation: Operation =
        Operation::from_bytes(hex::decode(operation_data)?).map_err(|e| {
            RpcServiceError::UnexpectedError {
                reason: format!("{}", e),
            }
        })?;
    let operation_hash: OperationHash = operation.message_typed_hash()?;

    let msg = RpcShellAutomatonMsg::InjectOperation {
        operation: operation.clone(),
        operation_hash: operation_hash.clone(),
        injected: start_request,
    };
    let receiver: tokio::sync::oneshot::Receiver<serde_json::Value> =
        env.shell_automaton_sender().send(msg).await.map_err(|_| {
            RpcServiceError::UnexpectedError {
                reason: "the channel between rpc and shell is overflown".to_string(),
            }
        })?;
    let operation_hash_b58check_string = operation_hash.to_base58_check();

    let join_handle = tokio::task::spawn(async move {
        let result = tokio::time::timeout(INJECT_OPERATION_WAIT_TIMEOUT, receiver).await;
        let result = match result {
            Ok(Ok(serde_json::Value::Null)) => Ok(operation_hash_b58check_string.clone()),
            Ok(Ok(serde_json::Value::String(reason))) => {
                Err(RpcServiceError::UnexpectedError { reason })
            }
            Ok(Ok(resp)) => Err(RpcServiceError::UnexpectedError {
                reason: resp.to_string(),
            }),
            Ok(Err(err)) => {
                warn!(
                    env.log(),
                    "Operation injection. State machine failed to respond: {}", err
                );
                Err(RpcServiceError::UnexpectedError {
                    reason: err.to_string(),
                })
            }
            Err(elapsed) => {
                warn!(env.log(), "Operation injection timeout"; "elapsed" => elapsed.to_string());
                Err(RpcServiceError::UnexpectedError {
                    reason: "timeout".to_string(),
                })
            }
        };

        info!(env.log(), "Operation injected";
            "operation_hash" => &operation_hash_b58check_string,
            "elapsed" => format!("{:?}", start_request.elapsed()));

        result
    });

    // Don't wait for injection to complete if `is_async` is enabled
    if is_async {
        Ok(operation_hash.to_base58_check())
    } else {
        match join_handle.await {
            Err(err) => Err(RpcServiceError::UnexpectedError {
                reason: err.to_string(),
            }),
            Ok(result) => result,
        }
    }
}

pub async fn inject_block(
    is_async: bool,
    chain_id: ChainId,
    injection_data: &str,
    env: &RpcServiceEnvironment,
) -> Result<String, RpcServiceError> {
    let block_with_op: InjectedBlockWithOperations = serde_json::from_str(injection_data)?;

    let start_request = Instant::now();

    let header: BlockHeaderWithHash = BlockHeader::from_bytes(hex::decode(block_with_op.data)?)
        .map_err(|e| RpcServiceError::UnexpectedError {
            reason: format!("{}", e),
        })?
        .try_into()?;
    let block_hash_b58check_string = header.hash.to_base58_check();
    info!(env.log(),
          "Block injection requested";
          "block_hash" => block_hash_b58check_string.clone(),
          "chain_id" => chain_id.to_base58_check(),
          "is_async" => is_async,
    );

    // special case for block on level 1 - has 0 validation passes
    let validation_passes: Option<Vec<Vec<Operation>>> = if header.header.validation_pass() > 0 {
        Some(
            block_with_op
                .operations
                .into_iter()
                .map(|validation_pass| {
                    validation_pass
                        .into_iter()
                        .map(|op| op.try_into())
                        .collect::<Result<_, _>>()
                })
                .collect::<Result<_, _>>()
                .map_err(|e| RpcServiceError::UnexpectedError {
                    reason: format!("{}", e),
                })?,
        )
    } else {
        None
    };

    // compute the paths for each validation passes
    let paths = if let Some(vps) = validation_passes.as_ref() {
        let mut connection = env.tezos_protocol_api().readable_connection().await?;

        let response = connection
            .compute_path(vps.try_into()?)
            .await
            .map_err(|e| RpcServiceError::UnexpectedError {
                reason: format!("{}", e),
            })?;
        Some(response.operations_hashes_path)
    } else {
        None
    };

    let start_async = Instant::now();

    let result = process_injected_block(
        env,
        chain_id.clone(),
        header,
        validation_passes,
        paths,
        start_request,
    )
    .await;

    match result {
        Ok(_) => {
            info!(env.log(),
                    "Block injected";
                    "block_hash" => block_hash_b58check_string.clone(),
                    "chain_id" => chain_id.to_base58_check(),
                    "elapsed" => format!("{:?}", start_request.elapsed()),
                    "elapsed_async" => format!("{:?}", start_async.elapsed()),
            );
        }
        Err(e) => {
            return Err(RpcServiceError::UnexpectedError {
                reason: format!(
                    "Block injection error received, block_hash: {}, reason: {}!",
                    &block_hash_b58check_string, e.reason
                ),
            });
        }
    }

    // return the block hash to the caller
    Ok(block_hash_b58check_string)
}

#[derive(Debug)]
pub struct InjectBlockError {
    pub reason: String,
}

impl From<StorageError> for InjectBlockError {
    fn from(error: StorageError) -> Self {
        Self {
            reason: format!("{:?}", error),
        }
    }
}

async fn process_injected_block(
    env: &RpcServiceEnvironment,
    chain_id: ChainId,
    block_header_with_hash: BlockHeaderWithHash,
    operations: Option<Vec<Vec<Operation>>>,
    operation_paths: Option<Vec<Path>>,
    injected: Instant,
) -> Result<(), InjectBlockError> {
    let log = env
        .log()
        .new(slog::o!("block" => block_header_with_hash.hash.to_base58_check(), "chain_id" => chain_id.to_base58_check()));

    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::InjectBlockStart {
            chain_id: chain_id.clone(),
            block_hash: block_header_with_hash.hash.clone(),
            block_header: block_header_with_hash.header.clone(),
            injected,
        })
        .await;
    // this should  allways return [is_new_block==true], as we are injecting a forged new block
    let (_, is_new_block, are_operations_complete) =
        match process_injected_block_header(env, &chain_id, &block_header_with_hash) {
            Ok(data) => data,
            Err(e) => {
                return Err(InjectBlockError {
                    reason: format!(
                        "Failed to store injected block, block_hash: {}, reason: {}",
                        block_header_with_hash.hash.to_base58_check(),
                        e
                    ),
                });
            }
        };
    info!(log, "New block injection";
               "is_new_block" => is_new_block,
               "level" => block_header_with_hash.header.level());

    if is_new_block {
        // handle operations (if expecting any)
        if !are_operations_complete {
            let operations = match operations {
                Some(operations) => operations,
                None => {
                    return Err(InjectBlockError {
                        reason: format!(
                            "Missing operations in request, block_hash: {}",
                            block_header_with_hash.hash.to_base58_check()
                        ),
                    });
                }
            };
            let op_paths = match operation_paths {
                Some(op_paths) => op_paths,
                None => {
                    return Err(InjectBlockError {
                        reason: format!(
                            "Missing operation paths in request, block_hash: {}",
                            block_header_with_hash.hash.to_base58_check()
                        ),
                    });
                }
            };

            // iterate through all validation passes
            for (idx, ops) in operations.into_iter().enumerate() {
                let opb = OperationsForBlock::new(block_header_with_hash.hash.clone(), idx as i8);

                // create OperationsForBlocksMessage - the operations are stored in DB as a OperationsForBlocksMessage per validation pass per block
                // e.g one block -> 4 validation passes -> 4 OperationsForBlocksMessage to store for the block
                let operation_hashes_path = match op_paths.get(idx) {
                    Some(path) => path.to_owned(),
                    None => {
                        return Err(InjectBlockError {
                            reason: format!(
                                "Missing operation paths in request for index: {}, block_hash: {}",
                                idx,
                                block_header_with_hash.hash.to_base58_check()
                            ),
                        });
                    }
                };

                let msg: OperationsForBlocksMessage =
                    OperationsForBlocksMessage::new(opb, operation_hashes_path, ops);

                match process_block_operations(env, &msg) {
                    Ok((all_operations_received, _)) => {
                        if all_operations_received {
                            info!(log, "New block injection - operations are complete";
                                       "is_new_block" => is_new_block,
                                       "level" => block_header_with_hash.header.level());
                        }
                    }
                    Err(e) => {
                        return Err(InjectBlockError {reason: format!("Failed to store injected block operations, block_hash: {}, reason: {}", block_header_with_hash.hash.to_base58_check(), e)});
                    }
                };
            }
        }

        // try apply block
        try_apply_block(env, block_header_with_hash.hash.clone()).await?;
    } else {
        warn!(log, "Injected duplicated block - will be ignored!");
        return Err(InjectBlockError {
            reason: format!(
                "Injected duplicated block - will be ignored!, block_hash: {}",
                block_header_with_hash.hash.to_base58_check()
            ),
        });
    }

    Ok(())
}

/// Process block_header, stores/updates storages, schedules missing stuff
///
/// Returns:
/// [metadata] - block header metadata
/// [is_new_block] - if it is a new block or previosly stored
/// [are_operations_complete] - if operations are completed
///
pub fn process_injected_block_header(
    env: &RpcServiceEnvironment,
    chain_id: &ChainId,
    block_header: &BlockHeaderWithHash,
) -> Result<(Meta, bool, bool), StorageError> {
    let block_storage = BlockStorage::new(env.persistent_storage());
    let block_meta_storage = BlockMetaStorage::new(env.persistent_storage());
    // store block
    let is_new_block = block_storage.put_block_header(block_header)?;

    // update block metadata
    let metadata = block_meta_storage.put_block_header(block_header, chain_id, env.log())?;

    // update operations metadata
    let are_operations_complete = process_injected_block_header_operations(env, block_header)?;

    Ok((metadata, is_new_block, are_operations_complete))
}

/// Process injected block header. This will create record in meta storage.
/// As the the header is injected via RPC, the operations are as well, so we
/// won't mark its operations as missing
///
/// Returns true, if validation_passes are completed (can happen, when validation_pass = 0)
fn process_injected_block_header_operations(
    env: &RpcServiceEnvironment,
    block_header: &BlockHeaderWithHash,
) -> Result<bool, StorageError> {
    let operations_meta_storage = OperationsMetaStorage::new(env.persistent_storage());
    match operations_meta_storage.get(&block_header.hash)? {
        Some(meta) => Ok(meta.is_complete()),
        None => operations_meta_storage
            .put_block_header(block_header)
            .map(|(is_complete, _)| is_complete),
    }
}

/// Process block operations. This will mark operations in store for the block as seen.
///
/// Returns tuple:
///     (
///         are_operations_complete,
///         missing_validation_passes
///     )
pub fn process_block_operations(
    env: &RpcServiceEnvironment,
    message: &OperationsForBlocksMessage,
) -> Result<(bool, Option<HashSet<u8>>), StorageError> {
    let operations_storage = OperationsStorage::new(env.persistent_storage());
    let operations_meta_storage = OperationsMetaStorage::new(env.persistent_storage());
    if operations_meta_storage.is_complete(message.operations_for_block().hash())? {
        return Ok((true, None));
    }

    operations_storage.put_operations(message)?;
    operations_meta_storage.put_operations(message)
}

pub async fn try_apply_block(
    env: &RpcServiceEnvironment,
    block_hash: BlockHash,
) -> Result<(), InjectBlockError> {
    let block_meta_storage = BlockMetaStorage::new(env.persistent_storage());
    let operations_meta_storage = OperationsMetaStorage::new(env.persistent_storage());

    // get block metadata
    let block_metadata = match block_meta_storage.get(&block_hash)? {
        Some(block_metadata) => block_metadata,
        None => {
            return Err(InjectBlockError {
                reason: format!(
                    "No metadata found for block_hash: {}",
                    block_hash.to_base58_check()
                ),
            });
        }
    };

    // check if can be applied
    match shell::validation::can_apply_block(
        (&block_hash, &block_metadata),
        |bh| operations_meta_storage.is_complete(bh),
        |predecessor| block_meta_storage.is_applied(predecessor),
    )? {
        CanApplyStatus::Ready => {}
        CanApplyStatus::AlreadyApplied => {
            return Err(InjectBlockError {
                reason: "Block cannot be applied, because it's is already applied".to_string(),
            });
        }
        CanApplyStatus::MissingPredecessor => {
            return Err(InjectBlockError {
                reason: format!(
                    "Block {} cannot be applied because missing predecessor block",
                    block_hash.to_base58_check()
                ),
            });
        }
        CanApplyStatus::PredecessorNotApplied => {
            return Err(InjectBlockError {
                reason: format!(
                    "Block {} cannot be applied because predecessor block is not applied yet",
                    block_hash.to_base58_check()
                ),
            });
        }
        CanApplyStatus::MissingOperations => {
            return Err(InjectBlockError {
                reason: format!(
                    "Block {} cannot be applied because missing operations",
                    block_hash.to_base58_check()
                ),
            });
        }
    }

    let result = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::InjectBlock {
            block_hash: block_hash.clone(),
        })
        .await;
    match result {
        Ok(result) => match result.await {
            Ok(serde_json::Value::Null) => Ok(()),
            Ok(serde_json::Value::String(error)) => Err(InjectBlockError {
                reason: format!(
                    "Block {} cannot be applied because block application failed. Reason: {}",
                    block_hash.to_base58_check(),
                    error,
                ),
            }),
            _ => Err(InjectBlockError {
                reason: format!(
                    "Block {} cannot be applied because unknown block application error.",
                    block_hash.to_base58_check(),
                ),
            }),
        },
        Err(_) => Err(InjectBlockError {
            reason: format!(
                "Block {} cannot be applied because sending block for application failed!",
                block_hash.to_base58_check(),
            ),
        }),
    }
}

pub async fn request_operations(env: &RpcServiceEnvironment) -> Result<(), RpcServiceError> {
    // request current head from the peers
    if let Err(err) = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::RequestCurrentHeadFromConnectedPeers)
        .await
    {
        warn!(env.log(), "state machine failed to respond: {}", err);
        return Err(RpcServiceError::UnexpectedError {
            reason: err.to_string(),
        });
    }

    Ok(())
}
