use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use failure::format_err;
use riker::actors::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slog::Logger;

use crypto::hash::{HashType, OperationHash, ProtocolHash};
use shell::shell_channel::{CurrentMempoolState, MempoolOperationReceived, ShellChannelRef, ShellChannelTopic, InjectBlock};
use storage::mempool_storage::MempoolOperationType;
use storage::MempoolStorage;
use storage::persistent::PersistentStorage;
use tezos_api::ffi::{Applied, Errored};
use tezos_messages::p2p::binary_message::{BinaryMessage, MessageHash};
use tezos_messages::p2p::encoding::prelude::{Operation, OperationMessage, BlockHeader};

use crate::rpc_actor::RpcCollectedStateRef;

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
    pub operations: Vec<Vec<Operation>>,
}

pub fn get_pending_operations(
    _persistent_storage: &PersistentStorage,
    state: &RpcCollectedStateRef,
    _log: &Logger) -> Result<MempoolOperations, failure::Error> {

    // get actual known state of mempool
    let state = state.read().unwrap();
    let current_mempool_state: &Option<CurrentMempoolState> = state.current_mempool_state();

    // convert to rpc data
    match current_mempool_state {
        Some(mempool) => {
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
    persistent_storage: &PersistentStorage,
    _state: &RpcCollectedStateRef,
    shell_channel: ShellChannelRef,
    _log: &Logger) -> Result<String, failure::Error> {
    let mut mempool_storage = MempoolStorage::new(persistent_storage);

    let operation: Operation = Operation::from_bytes(hex::decode(operation_data)?)?;
    let operation_message = OperationMessage::new(operation.clone());
    let ttl = SystemTime::now() + Duration::from_secs(60);
    let operation_hash = operation.message_hash()?;

    mempool_storage.put(MempoolOperationType::Pending, operation_message, ttl)?;

    shell_channel.tell(
        Publish {
            msg: MempoolOperationReceived {
                operation_hash: operation_hash.clone(),
                operation_type: MempoolOperationType::Pending,
            }.into(),
            topic: ShellChannelTopic::ShellEvents.into(),
        }, None);

    Ok(HashType::OperationHash.bytes_to_string(&operation_hash))
}

pub fn inject_block(
    injection_data: &str,
    persistent_storage: &PersistentStorage,
    _state: &RpcCollectedStateRef,
    shell_channel: ShellChannelRef,
    _log: &Logger) -> Result<String, failure::Error> {

    let injection_data_json: serde_json::Value = serde_json::from_str(injection_data)?;

    let header: BlockHeader = BlockHeader::from_bytes(hex::decode(injection_data_json["data"].to_string().replace("\"", ""))?)?;

    let injected_level = header.level();

    let block_hash = HashType::BlockHash.bytes_to_string(&header.message_hash().unwrap());
    let opers = if injected_level > 1 {
        let block_with_op: InjectedBlockWithOperations = serde_json::from_value(injection_data_json)?;
        Some(block_with_op.operations)
    } else {
        None
    };

    shell_channel.tell(
        Publish {
            msg: InjectBlock {
                block_header: header.clone(),
                operations: opers,
            }.into(),
            topic: ShellChannelTopic::ShellEvents.into(),
        }, None);

    // handle injected operations - WARNING - special case for level 1, when protocol is "activated"
    // if injected_level > 1 {
    //     println!("Op RAW: {:?}", injection_data_json["operations"].to_string());

    //     let block_with_op: InjectedBlockWithOperations = serde_json::from_value(injection_data_json)?;
    //     // let validation_passes = block_with_op.operations.len();

    //     // store operations to db
    //     let operations = block_with_op.operations.clone();
    //     for (idx, ops) in operations.iter().enumerate() {
    //         let opb = OperationsForBlock::new(header.message_hash().unwrap(), idx as i8);
    //         let msg: OperationsForBlocksMessage = OperationsForBlocksMessage::new(opb, Path::Op, ops.clone());
    //         operations_storage.put_operations(&msg)?;
    //         operations_meta_storage.put_operations(&msg)?;
    //     }
        
    //     //println!("Op: {:?}", operations);
    // } 

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