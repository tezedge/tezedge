// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use shell_automaton::service::rpc_service::RpcResponse as RpcShellAutomatonMsg;
use slog::{info, warn};

use crypto::hash::{ChainId, OperationHash, ProtocolHash};
use shell_integration::*;
use storage::BlockHeaderWithHash;
use tezos_api::ffi::{Applied, Errored};
use tezos_messages::p2p::binary_message::{BinaryRead, MessageHash};
use tezos_messages::p2p::encoding::operation::DecodedOperation;
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};

use crate::services::dev_services;
use crate::helpers::RpcServiceError;
use crate::server::RpcServiceEnvironment;

const INJECT_BLOCK_WAIT_TIMEOUT: Duration = Duration::from_secs(60);
const INJECT_OPERATION_WAIT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MempoolOperations {
    pub applied: Vec<HashMap<String, Value>>,
    pub refused: Vec<Value>,
    pub branch_refused: Vec<Value>,
    pub branch_delayed: Vec<Value>,
    // TODO: unprocessed - we dont have protocol data, because we can get it just from ffi now
    pub unprocessed: Vec<Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct InjectedBlockWithOperations {
    pub data: String,
    pub operations: Vec<Vec<DecodedOperation>>,
}

pub async fn get_pending_operations(
    _chain_id: &ChainId,
    env: Arc<RpcServiceEnvironment>,
) -> Result<(MempoolOperations, Option<ProtocolHash>), RpcServiceError> {
    // get actual known state of mempool
    let state = dev_services::get_shell_automaton_state_current(&env)
        .await
        .map_err(|_| RpcServiceError::UnexpectedError {
            reason: "too many requests, the channel between shell and rpc is full".to_string(),
        })?
        .mempool;

    Ok((
        MempoolOperations {
            applied: {
                state.applied_operations
                    .iter()
                    .map(|(operation_hash, operation)| {
                        let mut m = HashMap::new();
                        let hash = operation_hash.to_base58_check();
                        let branch = operation.branch().to_base58_check();
                        m.insert(String::from("hash"), Value::String(hash));
                        m.insert(String::from("branch"), Value::String(branch));
                        m
                    })
                    .collect()
            },
            refused: vec![],
            branch_refused: vec![],
            branch_delayed: vec![],
            unprocessed: vec![],
        },
        None,
    ))

    // convert to rpc data - we need protocol_hash
    /*let (mempool_operations, mempool_prevalidator_protocol) = match current_mempool_state
        .prevalidator()
    {
        Some(prevalidator) => {
            let result = current_mempool_state.result();
            let operations = current_mempool_state.operations();
            (
                MempoolOperations {
                    applied: convert_applied(&result.applied, operations)?,
                    refused: convert_errored(&result.refused, operations, &prevalidator.protocol)?,
                    branch_refused: convert_errored(
                        &result.branch_refused,
                        operations,
                        &prevalidator.protocol,
                    )?,
                    branch_delayed: convert_errored(
                        &result.branch_delayed,
                        operations,
                        &prevalidator.protocol,
                    )?,
                    unprocessed: vec![],
                },
                Some(prevalidator.protocol.clone()),
            )
        }
        None => (MempoolOperations::default(), None),
    };

    Ok((mempool_operations, mempool_prevalidator_protocol))*/
}

#[allow(dead_code)]
pub(crate) fn convert_applied(
    applied: &[Applied],
    operations: &HashMap<OperationHash, MempoolOperationRef>,
) -> Result<Vec<HashMap<String, Value>>, RpcServiceError> {
    let mut result: Vec<HashMap<String, Value>> = Vec::with_capacity(applied.len());
    for a in applied {
        let operation_hash = a.hash.to_base58_check();
        let protocol_data: HashMap<String, Value> = serde_json::from_str(&a.protocol_data_json)?;
        let operation = match operations.get(&a.hash) {
            Some(b) => b,
            None => {
                return Err(RpcServiceError::UnexpectedError {
                    reason: format!(
                        "missing operation data for operation_hash: {}",
                        &operation_hash
                    ),
                });
            }
        };

        let mut m = HashMap::new();
        m.insert(String::from("hash"), Value::String(operation_hash));
        m.insert(
            String::from("branch"),
            Value::String(operation.operation().branch().to_base58_check()),
        );
        m.extend(protocol_data);
        result.push(m);
    }

    Ok(result)
}

#[allow(dead_code)]
pub(crate) fn convert_errored(
    errored: &[Errored],
    operations: &HashMap<OperationHash, MempoolOperationRef>,
    protocol: &ProtocolHash,
) -> Result<Vec<Value>, RpcServiceError> {
    let mut result: Vec<Value> = Vec::with_capacity(errored.len());
    let protocol = protocol.to_base58_check();

    for e in errored {
        let operation_hash = e.hash.to_base58_check();
        let operation = match operations.get(&e.hash) {
            Some(b) => b,
            None => {
                return Err(RpcServiceError::UnexpectedError {
                    reason: format!(
                        "missing operation data for operation_hash: {}",
                        &operation_hash
                    ),
                });
            }
        };

        let protocol_data: HashMap<String, Value> = if e
            .protocol_data_json_with_error_json
            .protocol_data_json
            .is_empty()
        {
            HashMap::new()
        } else {
            serde_json::from_str(&e.protocol_data_json_with_error_json.protocol_data_json)?
        };

        let error = if e.protocol_data_json_with_error_json.error_json.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&e.protocol_data_json_with_error_json.error_json)?
        };

        let mut m = HashMap::new();
        m.insert(String::from("protocol"), Value::String(protocol.clone()));
        m.insert(
            String::from("branch"),
            Value::String(operation.operation().branch().to_base58_check()),
        );
        m.extend(protocol_data);
        m.insert(String::from("error"), error);

        result.push(Value::Array(vec![
            Value::String(operation_hash),
            serde_json::to_value(m)?,
        ]));
    }

    Ok(result)
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
    };
    let receiver: tokio::sync::oneshot::Receiver<serde_json::Value> = env.shell_automaton_sender()
        .send(msg)
        .await
        .map_err(|_| RpcServiceError::UnexpectedError {
            reason: "the channel between rpc and shell is overflown".to_string(),
        })?;
    let operation_hash_b58check_string = operation_hash.to_base58_check();

    let result = tokio::time::timeout(INJECT_OPERATION_WAIT_TIMEOUT, receiver).await;
    match result {
        // do we need the response
        Ok(Ok(_)) => (),
        Ok(Err(_)) => warn!(env.log(), "Operation injection, state machine failed to respond"),
        Err(elapsed) => {
            warn!(env.log(), "Operation injection timeout"; "elapsed" => elapsed.to_string());
            return Err(RpcServiceError::UnexpectedError { reason: "timeout".to_string() });
        },
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
        let mut current_mempool_state = env.current_mempool_state_storage().write()?;

        for vps in validation_passes {
            for vp in vps {
                let oph: OperationHash = vp.message_typed_hash()?;
                current_mempool_state.remove_operation(oph);
            }
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

pub fn request_operations(env: &RpcServiceEnvironment) -> Result<(), RpcServiceError> {
    // request current head from the peers
    env.shell_connector()
        .request_current_head_from_connected_peers();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::{collections::HashMap, convert::TryInto};

    use assert_json_diff::assert_json_eq;
    use serde_json::json;

    use tezos_api::ffi::{Applied, Errored, OperationProtocolDataJsonWithErrorListJson};
    use tezos_messages::p2p::binary_message::BinaryRead;
    use tezos_messages::p2p::encoding::prelude::{Operation, OperationMessage};

    use crate::services::mempool_services::{convert_applied, convert_errored};

    #[test]
    fn test_convert_applied() -> Result<(), anyhow::Error> {
        let data = vec![
            Applied {
                hash: "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into()?,
                protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
            }
        ];

        let mut operations = HashMap::new();
        // operation with branch=BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H
        operations.insert(
            "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into()?,
            Arc::new(OperationMessage::from(Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?)),
        );

        let expected_json = json!(
            [
                {
                    "hash" : "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ",
                    "branch" : "BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H",
                    "contents": [{ "kind": "endorsement", "level": 459020 } ],
                    "signature": "siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9"
                }
            ]
        );

        // convert
        let result = convert_applied(&data, &operations)?;
        assert_json_eq!(
            serde_json::to_value(result)?,
            serde_json::to_value(expected_json)?
        );

        Ok(())
    }

    #[test]
    fn test_convert_errored() -> Result<(), anyhow::Error> {
        let data = vec![
            Errored {
                hash: "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into()?,
                is_endorsement: None,
                protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson {
                    protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
                    error_json: "[ { \"kind\": \"temporary\",\n    \"id\": \"proto.005-PsBabyM1.operation.wrong_endorsement_predecessor\",\n    \"expected\": \"BMDb9PfcJmiibDDEbd6bEEDj4XNG4C7QACG6TWqz29c9FxNgDLL\",\n    \"provided\": \"BLd8dLs4X5Ve6a8B37kUu7iJkRycWzfSF5MrskY4z8YaideQAp4\" } ]".to_string(),
                },
            }
        ];

        let mut operations = HashMap::new();
        // operation with branch=BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H
        operations.insert(
            "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into()?,
            Arc::new(OperationMessage::from(Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?)),
        );
        let protocol = "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb".try_into()?;

        let expected_json = json!(
                [
                    [
                        "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ",
                        {
                            "protocol" : "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb",
                            "branch" : "BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H",
                            "contents": [{ "kind": "endorsement", "level": 459020}],
                            "signature": "siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9",
                            "error" : [ { "kind": "temporary", "id": "proto.005-PsBabyM1.operation.wrong_endorsement_predecessor", "expected": "BMDb9PfcJmiibDDEbd6bEEDj4XNG4C7QACG6TWqz29c9FxNgDLL", "provided": "BLd8dLs4X5Ve6a8B37kUu7iJkRycWzfSF5MrskY4z8YaideQAp4" } ]
                        }
                    ]
                ]
        );

        // convert
        let result = convert_errored(&data, &operations, &protocol)?;
        assert_json_eq!(
            serde_json::to_value(result)?,
            serde_json::to_value(expected_json)?
        );

        Ok(())
    }

    #[test]
    fn test_convert_errored_missing_protocol_data() -> Result<(), anyhow::Error> {
        let data = vec![
            Errored {
                hash: "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into()?,
                is_endorsement: Some(true),
                protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson {
                    protocol_data_json: "".to_string(),
                    error_json: "[ { \"kind\": \"temporary\",\n    \"id\": \"proto.005-PsBabyM1.operation.wrong_endorsement_predecessor\",\n    \"expected\": \"BMDb9PfcJmiibDDEbd6bEEDj4XNG4C7QACG6TWqz29c9FxNgDLL\",\n    \"provided\": \"BLd8dLs4X5Ve6a8B37kUu7iJkRycWzfSF5MrskY4z8YaideQAp4\" } ]".to_string(),
                },
            }
        ];

        let mut operations = HashMap::new();
        // operation with branch=BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H
        operations.insert(
            "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into()?,
            Arc::new(OperationMessage::from(Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?)),
        );
        let protocol = "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb".try_into()?;

        let expected_json = json!(
                [
                    [
                        "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ",
                        {
                            "protocol" : "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb",
                            "branch" : "BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H",
                            "error" : [ { "kind": "temporary", "id": "proto.005-PsBabyM1.operation.wrong_endorsement_predecessor", "expected": "BMDb9PfcJmiibDDEbd6bEEDj4XNG4C7QACG6TWqz29c9FxNgDLL", "provided": "BLd8dLs4X5Ve6a8B37kUu7iJkRycWzfSF5MrskY4z8YaideQAp4" } ]
                        }
                    ]
                ]
        );

        // convert
        let result = convert_errored(&data, &operations, &protocol)?;
        assert_json_eq!(
            serde_json::to_value(result)?,
            serde_json::to_value(expected_json)?
        );

        Ok(())
    }
}
