// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use failure::{bail, format_err};
use riker::actors::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crypto::hash::{ChainId, HashType, OperationHash, ProtocolHash};
use shell::mempool::CurrentMempoolStateStorageRef;
use shell::shell_channel::{InjectBlock, MempoolOperationReceived, RequestCurrentHead, ShellChannelRef, ShellChannelTopic};
use shell::validation;
use storage::{BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader, MempoolStorage};
use storage::mempool_storage::MempoolOperationType;
use tezos_api::ffi::{Applied, ComputePathRequest, Errored};
use tezos_messages::p2p::binary_message::{BinaryMessage, MessageHash};
use tezos_messages::p2p::encoding::operation::DecodedOperation;
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};

use crate::helpers::get_prevalidators;
use crate::server::RpcServiceEnvironment;

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

pub fn get_pending_operations(
    _chain_id: &ChainId,
    current_mempool_state_storage: CurrentMempoolStateStorageRef,
) -> Result<(MempoolOperations, Option<ProtocolHash>), failure::Error> {

    // get actual known state of mempool
    let current_mempool_state = current_mempool_state_storage
        .read()
        .map_err(|e| format_err!("Failed to obtain read lock, reson: {}", e))?;

    // convert to rpc data - we need protocol_hash
    let (mempool_operations, mempool_prevalidator_protocol) = match current_mempool_state.prevalidator() {
        Some(prevalidator) => {
            let result = current_mempool_state.result();
            let operations = current_mempool_state.operations();
            (
                MempoolOperations {
                    applied: convert_applied(&result.applied, &operations)?,
                    refused: convert_errored(&result.refused, &operations, &prevalidator.protocol)?,
                    branch_refused: convert_errored(&result.branch_refused, &operations, &prevalidator.protocol)?,
                    branch_delayed: convert_errored(&result.branch_delayed, &operations, &prevalidator.protocol)?,
                    unprocessed: vec![],
                },
                Some(prevalidator.protocol.clone()),
            )
        }
        None => (MempoolOperations::default(), None)
    };

    Ok(
        (
            mempool_operations,
            mempool_prevalidator_protocol
        )
    )
}

fn convert_applied(applied: &Vec<Applied>, operations: &HashMap<OperationHash, Operation>) -> Result<Vec<HashMap<String, Value>>, failure::Error> {
    let mut result: Vec<HashMap<String, Value>> = Vec::new();
    for a in applied {
        let operation_hash = HashType::OperationHash.hash_to_b58check(&a.hash);
        let protocol_data: HashMap<String, Value> = serde_json::from_str(&a.protocol_data_json)?;
        let operation = match operations.get(&a.hash) {
            Some(b) => b,
            None => return Err(format_err!("missing operation data for operation_hash: {}", &operation_hash))
        };

        let mut m = HashMap::new();
        m.insert(String::from("hash"), Value::String(operation_hash));
        m.insert(String::from("branch"), Value::String(HashType::BlockHash.hash_to_b58check(&operation.branch())));
        m.extend(protocol_data);
        result.push(m);
    }

    Ok(result)
}

fn convert_errored(errored: &Vec<Errored>, operations: &HashMap<OperationHash, Operation>, protocol: &ProtocolHash) -> Result<Vec<Value>, failure::Error> {
    let mut result: Vec<Value> = Vec::new();
    let protocol = HashType::ProtocolHash.hash_to_b58check(&protocol);

    for e in errored {
        let operation_hash = HashType::OperationHash.hash_to_b58check(&e.hash);
        let operation = match operations.get(&e.hash) {
            Some(b) => b,
            None => return Err(format_err!("missing operation data for operation_hash: {}", &operation_hash))
        };

        let protocol_data: HashMap<String, Value> = if e.protocol_data_json_with_error_json.protocol_data_json.is_empty() {
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
        m.insert(String::from("branch"), Value::String(HashType::BlockHash.hash_to_b58check(&operation.branch())));
        m.extend(protocol_data);
        m.insert(String::from("error"), error);

        result.push(
            Value::Array(
                vec![
                    Value::String(operation_hash),
                    serde_json::to_value(m)?,
                ]
            )
        );
    }

    Ok(result)
}

pub fn inject_operation(
    chain_id: ChainId,
    operation_data: &str,
    env: &RpcServiceEnvironment,
    shell_channel: &ShellChannelRef) -> Result<String, failure::Error> {
    let persistent_storage = env.persistent_storage();
    let block_storage: Box<dyn BlockStorageReader> = Box::new(BlockStorage::new(persistent_storage));
    let block_meta_storage: Box<dyn BlockMetaStorageReader> = Box::new(BlockMetaStorage::new(persistent_storage));

    // find prevalidator for chain_id, if not found, then stop
    if get_prevalidators(env, Some(&chain_id))?.is_empty() {
        bail!("Prevalidator is not running, cannot inject the operation.");
    }

    // parse operation data
    let operation: Operation = Operation::from_bytes(hex::decode(operation_data)?)?;
    let operation_hash = operation.message_hash()?;

    // do prevalidation before add the operation to mempool
    let result = validation::prevalidate_operation(
        &chain_id,
        &operation_hash,
        &operation,
        env.current_mempool_state_storage().clone(),
        &env.tezos_readonly_prevalidation_api().pool.get()?.api,
        &block_storage,
        &block_meta_storage,
    )?;

    // can accpect operation ?
    if !validation::can_accept_operation_from_rpc(&operation_hash, &result) {
        return Err(format_err!("Operation from rpc ({}) was not added to mempool. Reason: {:?}", HashType::OperationHash.hash_to_b58check(&operation_hash), result));
    }

    // store operation in mempool storage
    let mut mempool_storage = MempoolStorage::new(persistent_storage);
    let operation_hash_as_string = HashType::OperationHash.hash_to_b58check(&operation_hash);
    let ttl = SystemTime::now() + Duration::from_secs(60);
    mempool_storage.put(MempoolOperationType::Pending, operation.into(), ttl)?;

    // ping mempool with new operation for mempool validation
    shell_channel.tell(
        Publish {
            msg: MempoolOperationReceived {
                operation_hash,
                operation_type: MempoolOperationType::Pending,
            }.into(),
            topic: ShellChannelTopic::ShellEvents.into(),
        }, None);

    Ok(operation_hash_as_string)
}

pub fn inject_block(
    _chain_id: ChainId,
    injection_data: &str,
    env: &RpcServiceEnvironment,
    shell_channel: &ShellChannelRef) -> Result<String, failure::Error> {
    let block_with_op: InjectedBlockWithOperations = serde_json::from_str(injection_data)?;

    let header: BlockHeader = BlockHeader::from_bytes(hex::decode(block_with_op.data)?)?;
    let block_hash = HashType::BlockHash.hash_to_b58check(&header.message_hash()?);

    // special case for block on level 1 - has 0 validation passes
    let validation_passes: Option<Vec<Vec<Operation>>> = if header.validation_pass() > 0 {
        Some(block_with_op.operations.into_iter()
            .map(|validation_pass| validation_pass.into_iter()
                .map(|op| op.into())
                .collect())
            .collect())
    } else {
        None
    };

    // clean actual mempool_state - just applied should be enough
    if let Some(validation_passes) = &validation_passes {
        let mut current_mempool_state = env.current_mempool_state_storage()
            .write()
            .map_err(|e| format_err!("Failed to obtain write lock, reason: {}", e))?;

        for vps in validation_passes {
            for vp in vps {
                let oph: OperationHash = vp.message_hash()?;
                current_mempool_state.remove_operation(oph);
            }
        }
    }

    // compute the paths for each validation passes
    let paths = if let Some(vps) = validation_passes.clone() {
        let request = ComputePathRequest {
            operations: vps.iter().map(|validation_pass| validation_pass.iter().map(|op| op.message_hash().unwrap()).collect()).collect(),
        };

        let response = env.tezos_without_context_api().pool.get()?.api.compute_path(request)?;
        Some(response.operations_hashes_path)
    } else {
        None
    };

    // notify other actors, that a block was injected
    shell_channel.tell(
        Publish {
            msg: InjectBlock {
                block_header: header,
                operations: validation_passes,
                operation_paths: paths,
            }.into(),
            topic: ShellChannelTopic::ShellEvents.into(),
        }, None);

    // return the block hash to the caller
    Ok(block_hash)
}

pub fn request_operations(shell_channel: ShellChannelRef) -> Result<(), failure::Error> {
    // request current head from the peers
    shell_channel.tell(
        Publish {
            msg: RequestCurrentHead.into(),
            topic: ShellChannelTopic::ShellEvents.into(),
        }, None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use assert_json_diff::assert_json_eq;
    use serde_json::json;

    use crypto::hash::HashType;
    use tezos_api::ffi::{Applied, Errored, OperationProtocolDataJsonWithErrorListJson};
    use tezos_messages::p2p::binary_message::BinaryMessage;
    use tezos_messages::p2p::encoding::prelude::Operation;

    use crate::services::mempool_services::{convert_applied, convert_errored};

    #[test]
    fn test_convert_applied() -> Result<(), failure::Error> {
        let data = vec![
            Applied {
                hash: HashType::OperationHash.b58check_to_hash("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
                protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
            }
        ];

        let mut operations = HashMap::new();
        // operation with branch=BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H
        operations.insert(
            HashType::OperationHash.b58check_to_hash("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
            Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?,
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
    fn test_convert_errored() -> Result<(), failure::Error> {
        let data = vec![
            Errored {
                hash: HashType::OperationHash.b58check_to_hash("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
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
            HashType::OperationHash.b58check_to_hash("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
            Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?,
        );
        let protocol = HashType::ProtocolHash.b58check_to_hash("PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb")?;

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
    fn test_convert_errored_missing_protocol_data() -> Result<(), failure::Error> {
        let data = vec![
            Errored {
                hash: HashType::OperationHash.b58check_to_hash("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
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
            HashType::OperationHash.b58check_to_hash("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
            Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?,
        );
        let protocol = HashType::ProtocolHash.b58check_to_hash("PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb")?;

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