// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crypto::hash::{BlockHash, HashBase58, OperationHash, ProtocolHash};
use tezos_api::ffi::{Applied, Errored};
use tezos_messages::p2p::encoding::operation::Operation;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MempoolOperations {
    applied: Vec<HashMap<String, Value>>,
    refused: Vec<Value>,
    branch_refused: Vec<Value>,
    branch_delayed: Vec<Value>,
    // TODO: unprocessed - we don't have protocol data, because we can get it just from ffi now
    unprocessed: Vec<Value>,
}

fn convert_applied(
    applied: &[Applied],
    operations: &HashMap<HashBase58<OperationHash>, Operation>,
    current_branch: &BlockHash,
) -> Vec<HashMap<String, Value>> {
    applied
        .iter()
        .filter_map(move |v| {
            let branch = operations.get(&v.hash)?.branch();
            if branch.ne(current_branch) {
                return None;
            }
            let mut m = serde_json::from_str(&v.protocol_data_json).unwrap_or_else(|err| {
                let mut m = HashMap::new();
                m.insert(
                    "protocol_data_parse_error".to_string(),
                    Value::String(err.to_string()),
                );
                m
            });
            m.insert("hash".to_string(), Value::String(v.hash.to_base58_check()));
            m.insert(
                "branch".to_string(),
                Value::String(branch.to_base58_check()),
            );
            Some(m)
        })
        .collect()
}

fn convert_errored(
    errored: &[Errored],
    operations: &HashMap<HashBase58<OperationHash>, Operation>,
    protocol: &ProtocolHash,
) -> Vec<Value> {
    let mut result = Vec::with_capacity(errored.len());
    for v in errored {
        let operation = match operations.get(&v.hash) {
            Some(b) => b,
            None => continue,
        };
        let mut m: HashMap<String, Value> = if v
            .protocol_data_json_with_error_json
            .protocol_data_json
            .is_empty()
        {
            HashMap::new()
        } else {
            serde_json::from_str(&v.protocol_data_json_with_error_json.protocol_data_json)
                .unwrap_or_else(|err| {
                    let mut m = HashMap::new();
                    m.insert(
                        "protocol_data_parse_error".to_string(),
                        Value::String(err.to_string()),
                    );
                    m
                })
        };

        let error = if v.protocol_data_json_with_error_json.error_json.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&v.protocol_data_json_with_error_json.error_json)
                .unwrap_or_else(|err| Value::String(err.to_string()))
        };

        m.insert(
            "protocol".to_string(),
            Value::String(protocol.to_base58_check()),
        );
        m.insert(
            "branch".to_string(),
            Value::String(operation.branch().to_base58_check()),
        );
        m.insert("error".to_string(), error);
        if let Ok(json) = serde_json::to_value(m) {
            result.push(Value::Array(vec![
                Value::String(v.hash.to_base58_check()),
                json,
            ]));
        }
    }
    result
}

impl MempoolOperations {
    pub fn collect(
        applied: &[Applied],
        refused: &[Errored],
        branch_delayed: &[Errored],
        branch_refused: &[Errored],
        operations: &HashMap<HashBase58<OperationHash>, Operation>,
        current_branch: &BlockHash,
        protocol: &ProtocolHash,
    ) -> Self {
        MempoolOperations {
            applied: convert_applied(applied, operations, current_branch),
            refused: convert_errored(refused, operations, protocol),
            branch_delayed: convert_errored(branch_delayed, operations, protocol),
            branch_refused: convert_errored(branch_refused, operations, protocol),
            unprocessed: vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitoredOperation<'a> {
    branch: String,
    #[serde(flatten)]
    protocol_data: HashMap<String, Value>,
    protocol: &'a str,
    hash: String,
    error: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol_data_parse_error: Option<String>,
}

impl<'a> MonitoredOperation<'a> {
    pub fn collect_applied(
        applied: &'a [Applied],
        operations: &'a HashMap<HashBase58<OperationHash>, Operation>,
        protocol_hash: &'a str,
    ) -> impl Iterator<Item = MonitoredOperation<'a>> + 'a {
        applied.iter().filter_map(move |applied_op| {
            let op_hash = applied_op.hash.to_base58_check();
            let operation = operations.get(&applied_op.hash)?;
            let (protocol_data, err) = match serde_json::from_str(&applied_op.protocol_data_json) {
                Ok(protocol_data) => (protocol_data, None),
                Err(err) => (HashMap::default(), Some(err.to_string())),
            };
            Some(MonitoredOperation {
                branch: operation.branch().to_base58_check(),
                protocol: protocol_hash,
                hash: op_hash,
                protocol_data,
                error: vec![],
                protocol_data_parse_error: err,
            })
        })
    }

    pub fn collect_errored(
        errored: &'a [Errored],
        operations: &'a HashMap<HashBase58<OperationHash>, Operation>,
        protocol_hash: &'a str,
    ) -> impl Iterator<Item = MonitoredOperation<'a>> + 'a {
        errored.iter().filter_map(move |errored_op| {
            let op_hash = errored_op.hash.to_base58_check();
            let operation = operations.get(&errored_op.hash)?;
            let json = &errored_op
                .protocol_data_json_with_error_json
                .protocol_data_json;
            let (protocol_data, err) = match serde_json::from_str(json) {
                Ok(protocol_data) => (protocol_data, None),
                Err(err) => (HashMap::default(), Some(err.to_string())),
            };
            let ocaml_err = &errored_op.protocol_data_json_with_error_json.error_json;
            Some(MonitoredOperation {
                branch: operation.branch().to_base58_check(),
                protocol: protocol_hash,
                hash: op_hash,
                protocol_data,
                error: serde_json::from_str(ocaml_err).unwrap_or_else(|err| vec![err.to_string()]),
                protocol_data_parse_error: err,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, convert::TryInto};

    use assert_json_diff::assert_json_eq;
    use serde_json::json;

    use tezos_api::ffi::{Applied, Errored, OperationProtocolDataJsonWithErrorListJson};
    use tezos_messages::p2p::binary_message::BinaryRead;
    use tezos_messages::p2p::encoding::operation::Operation;

    use super::{convert_applied, convert_errored};

    #[test]
    fn test_convert_applied() {
        let data = vec![
            Applied {
                hash: "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into().unwrap(),
                protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
            }
        ];

        let mut operations = HashMap::new();
        // operation with branch=BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H
        operations.insert(
            "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into().unwrap(),
            Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08").unwrap()).unwrap(),
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
        let current_branch = "BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H"
            .try_into()
            .unwrap();
        let result = convert_applied(&data, &operations, &current_branch);
        assert_json_eq!(serde_json::to_value(result).unwrap(), expected_json,);
    }

    #[test]
    fn test_convert_errored() {
        let data = vec![
            Errored {
                hash: "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into().unwrap(),
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
            "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into().unwrap(),
            Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08").unwrap()).unwrap(),
        );
        let protocol = "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb"
            .try_into()
            .unwrap();

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
        let result = convert_errored(&data, &operations, &protocol);
        assert_json_eq!(serde_json::to_value(result).unwrap(), expected_json,);
    }

    #[test]
    fn test_convert_errored_missing_protocol_data() {
        let data = vec![
            Errored {
                hash: "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into().unwrap(),
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
            "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into().unwrap(),
            Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08").unwrap()).unwrap(),
        );
        let protocol = "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb"
            .try_into()
            .unwrap();

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
        let result = convert_errored(&data, &operations, &protocol);
        assert_json_eq!(serde_json::to_value(result).unwrap(), expected_json,);
    }
}
