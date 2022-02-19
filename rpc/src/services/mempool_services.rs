// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use shell_automaton::service::rpc_service::RpcRequest as RpcShellAutomatonMsg;
use slog::{info, warn};

use crypto::hash::{ChainId, OperationHash};
use shell_integration::*;
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::binary_message::{BinaryRead, MessageHash};
use tezos_messages::p2p::encoding::operation::DecodedOperation;
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};

use crate::helpers::RpcServiceError;
use crate::server::RpcServiceEnvironment;

const INJECT_BLOCK_WAIT_TIMEOUT: Duration = Duration::from_secs(60);
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
    env: &RpcServiceEnvironment,
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

    let result = tokio::time::timeout(INJECT_OPERATION_WAIT_TIMEOUT, receiver).await;
    match result {
        Ok(Ok(serde_json::Value::Null)) => (),
        Ok(Ok(serde_json::Value::String(reason))) => {
            return Err(RpcServiceError::UnexpectedError { reason });
        }
        Ok(Ok(resp)) => {
            return Err(RpcServiceError::UnexpectedError {
                reason: resp.to_string(),
            });
        }
        Ok(Err(err)) => {
            warn!(
                env.log(),
                "Operation injection. State machine failed to respond: {}", err
            );
            return Err(RpcServiceError::UnexpectedError {
                reason: err.to_string(),
            });
        }
        Err(elapsed) => {
            warn!(env.log(), "Operation injection timeout"; "elapsed" => elapsed.to_string());
            return Err(RpcServiceError::UnexpectedError {
                reason: "timeout".to_string(),
            });
        }
    }

    info!(env.log(), "Operation injected";
        "operation_hash" => &operation_hash_b58check_string,
        "elapsed" => format!("{:?}", start_request.elapsed()));

    Ok(operation_hash_b58check_string)
}

pub async fn inject_block(
    is_async: bool,
    chain_id: ChainId,
    injection_data: &str,
    env: &RpcServiceEnvironment,
) -> Result<String, RpcServiceError> {
    let block_with_op: InjectedBlockWithOperations = serde_json::from_str(injection_data)?;
    let chain_id = Arc::new(chain_id);

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

    // clean actual mempool_state - just applied should be enough
    if let Some(validation_passes) = &validation_passes {
        let mut operation_hashes = Vec::new();
        for op in validation_passes.into_iter().flatten() {
            operation_hashes.push(op.message_typed_hash()?);
        }

        if let Err(err) = env
            .shell_automaton_sender()
            .send(RpcShellAutomatonMsg::RemoveOperations { operation_hashes })
            .await
        {
            warn!(env.log(), "state machine failed to remove ops: {}", err);
        }
    }

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

    // callback will wait all the asynchonous processing to finish, and then returns rpc response
    let (result_callback_sender, result_callback_receiver) = if is_async {
        // if async no wait
        (None, None)
    } else {
        // if not async, means sync and we wait till operations is added to pendings
        let (result_callback_sender, result_callback_receiver) = create_oneshot_callback();
        (Some(result_callback_sender), Some(result_callback_receiver))
    };

    if let Err(err) = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::InjectBlock {
            chain_id: chain_id.as_ref().clone(),
            block_hash: header.hash.clone(),
            block_header: header.header.clone(),
            injected: start_request,
        })
        .await
    {
        warn!(env.log(), "state machine failed to remove ops: {}", err);
    }

    let start_async = Instant::now();

    // notify other actors, that a block was injected
    env.shell_connector().inject_block(
        InjectBlock {
            chain_id: chain_id.clone(),
            block_header: Arc::new(header),
            operations: validation_passes,
            operation_paths: paths,
        },
        result_callback_sender,
    );

    // wait for result
    if let Some(receiver) = result_callback_receiver {
        // we spawn as blocking because we are under async/await
        let result =
            tokio::task::spawn_blocking(move || receiver.recv_timeout(INJECT_BLOCK_WAIT_TIMEOUT))
                .await;
        match result {
            Ok(Ok(_)) => {
                info!(env.log(),
                      "Block injected";
                      "block_hash" => block_hash_b58check_string.clone(),
                      "chain_id" => chain_id.to_base58_check(),
                      "elapsed" => format!("{:?}", start_request.elapsed()),
                      "elapsed_async" => format!("{:?}", start_async.elapsed()),
                );
            }
            Ok(Err(e)) => {
                return Err(RpcServiceError::UnexpectedError {
                    reason: format!(
                        "Block injection error received, block_hash: {}, reason: {}!",
                        &block_hash_b58check_string, e
                    ),
                });
            }
            Err(e) => {
                return Err(RpcServiceError::UnexpectedError {
                    reason: format!(
                        "Block injection error async wait, block_hash: {}, reason: {}!",
                        &block_hash_b58check_string, e
                    ),
                });
            }
        }
    }

    // return the block hash to the caller
    Ok(block_hash_b58check_string)
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
