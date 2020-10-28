use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use failure::format_err;
use riker::actors::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slog::Logger;

use crypto::hash::{HashType, OperationHash, ProtocolHash};
use shell::shell_channel::{CurrentMempoolState, InjectBlock, MempoolOperationReceived, ShellChannelRef, ShellChannelTopic};
use shell::validation;
use storage::{BlockStorage, BlockStorageReader, MempoolStorage};
use storage::mempool_storage::MempoolOperationType;
use tezos_api::ffi::{Applied, ComputePathRequest, Errored};
use tezos_messages::p2p::binary_message::{BinaryMessage, MessageHash};
use tezos_messages::p2p::encoding::operation::DecodedOperation;
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};

use crate::rpc_actor::{RpcCollectedState, RpcCollectedStateRef};
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
    state: &RpcCollectedStateRef,
    _log: &Logger) -> Result<MempoolOperations, failure::Error> {

    // get actual known state of mempool
    let state = state.read().unwrap();
    let current_mempool_state: &Option<Arc<RwLock<CurrentMempoolState>>> = state.current_mempool_state();

    // convert to rpc data
    match current_mempool_state {
        Some(mempool) => {
            let mempool = mempool.read().unwrap();
            let protocol = match &mempool.protocol {
                Some(protocol) => protocol,
                None => return Err(format_err!("missing protocol for mempool current state"))
            };

            Ok(MempoolOperations {
                applied: convert_applied(&mempool.result.applied, &mempool.operations)?,
                refused: convert_errored(&mempool.result.refused, &mempool.operations, &protocol)?,
                branch_refused: convert_errored(&mempool.result.branch_refused, &mempool.operations, &protocol)?,
                branch_delayed: convert_errored(&mempool.result.branch_delayed, &mempool.operations, &protocol)?,
                unprocessed: vec![],
            })
        }
        None => Ok(MempoolOperations::default())
    }
}

fn convert_applied(applied: &Vec<Applied>, operations: &HashMap<OperationHash, Operation>) -> Result<Vec<HashMap<String, Value>>, failure::Error> {
    let mut result: Vec<HashMap<String, Value>> = Vec::new();
    for a in applied {
        let operation_hash = HashType::OperationHash.bytes_to_string(&a.hash);
        let protocol_data: HashMap<String, Value> = serde_json::from_str(&a.protocol_data_json)?;
        let operation = match operations.get(&a.hash) {
            Some(b) => b,
            None => return Err(format_err!("missing operation data for operation_hash: {}", &operation_hash))
        };

        let mut m = HashMap::new();
        m.insert(String::from("hash"), Value::String(operation_hash));
        m.insert(String::from("branch"), Value::String(HashType::BlockHash.bytes_to_string(&operation.branch())));
        m.extend(protocol_data);
        result.push(m);
    }

    Ok(result)
}

fn convert_errored(errored: &Vec<Errored>, operations: &HashMap<OperationHash, Operation>, protocol: &ProtocolHash) -> Result<Vec<Value>, failure::Error> {
    let mut result: Vec<Value> = Vec::new();
    let protocol = HashType::ProtocolHash.bytes_to_string(&protocol);

    for e in errored {
        let operation_hash = HashType::OperationHash.bytes_to_string(&e.hash);
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
        m.insert(String::from("branch"), Value::String(HashType::BlockHash.bytes_to_string(&operation.branch())));
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
    operation_data: &str,
    env: &RpcServiceEnvironment,
    shell_channel: ShellChannelRef) -> Result<String, failure::Error> {
    let persistent_storage = env.persistent_storage();
    let block_storage: Box<dyn BlockStorageReader> = Box::new(BlockStorage::new(persistent_storage));
    let state = env.state();

    // parse operation data
    let operation: Operation = Operation::from_bytes(hex::decode(operation_data)?)?;
    let operation_hash = operation.message_hash()?;
    let state = state.read().unwrap();

    // do prevalidation before add the operation to mempool
    let result = validation::prevalidate_operation(
        state.chain_id(),
        &operation_hash,
        &operation,
        state.current_mempool_state(),
        &env.tezos_readonly_prevalidation_api().pool.get()?.api,
        &block_storage,
    )?;

    // can accpect operation ?
    if !validation::can_accept_operation_from_rpc(&operation_hash, &result) {
        return Err(format_err!("Operation from rpc ({}) was not added to mempool. Reason: {:?}", HashType::OperationHash.bytes_to_string(&operation_hash), result))
    }

    // store operation in mempool storage
    let mut mempool_storage = MempoolStorage::new(persistent_storage);
    let operation_hash_as_string = HashType::OperationHash.bytes_to_string(&operation_hash);
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
    injection_data: &str,
    env: &RpcServiceEnvironment,
    shell_channel: ShellChannelRef) -> Result<String, failure::Error> {
    let block_with_op: InjectedBlockWithOperations = serde_json::from_str(injection_data)?;

    let header: BlockHeader = BlockHeader::from_bytes(hex::decode(block_with_op.data)?)?;
    let block_hash = HashType::BlockHash.bytes_to_string(&header.message_hash()?);

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
        let current_head_ref: &mut RpcCollectedState = &mut *env.state().write().unwrap();
        if let Some(mempool) = current_head_ref.current_mempool_state() {
            let mut mempool = mempool.write().unwrap();
            for vps in validation_passes {
                for vp in vps {
                    let oph: OperationHash = vp.message_hash()?;

                    // remove from applied
                    if let Some(pos) = mempool.result.applied.iter().position(|x| oph.eq(&x.hash)) {
                        mempool.result.applied.remove(pos);
                        mempool.operations.remove(&oph);
                    }
                    // remove from branch_delayed
                    if let Some(pos) = mempool.result.branch_delayed.iter().position(|x| oph.eq(&x.hash)) {
                        mempool.result.branch_delayed.remove(pos);
                        mempool.operations.remove(&oph);
                    }
                    // remove from branch_refused
                    if let Some(pos) = mempool.result.branch_refused.iter().position(|x| oph.eq(&x.hash)) {
                        mempool.result.branch_refused.remove(pos);
                        mempool.operations.remove(&oph);
                    }
                    // remove from refused
                    if let Some(pos) = mempool.result.refused.iter().position(|x| oph.eq(&x.hash)) {
                        mempool.result.refused.remove(pos);
                        mempool.operations.remove(&oph);
                    }
                }
            }
        }
    }

    // compute the paths for each validation passes
    let paths = if let Some(vps) = validation_passes.clone() {
        let request = ComputePathRequest {
            operations: vps.clone().iter().map(|validation_pass| validation_pass.iter().map(|op| op.message_hash().unwrap()).collect()).collect(),
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
                hash: HashType::OperationHash.string_to_bytes("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
                protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
            }
        ];

        let mut operations = HashMap::new();
        // operation with branch=BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H
        operations.insert(
            HashType::OperationHash.string_to_bytes("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
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
                hash: HashType::OperationHash.string_to_bytes("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
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
            HashType::OperationHash.string_to_bytes("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
            Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?,
        );
        let protocol = HashType::ProtocolHash.string_to_bytes("PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb")?;

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
                hash: HashType::OperationHash.string_to_bytes("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
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
            HashType::OperationHash.string_to_bytes("onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ")?,
            Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?,
        );
        let protocol = HashType::ProtocolHash.string_to_bytes("PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb")?;

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