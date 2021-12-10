// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use serde::{Serialize, Deserialize};
use serde_json::Value;

use crypto::hash::OperationHash;
use tezos_api::ffi::{Applied, Errored};
use tezos_messages::p2p::encoding::operation::Operation;

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
        operations: &'a HashMap<OperationHash, Operation>,
        protocol_hash: &'a str,
    ) -> impl Iterator<Item = MonitoredOperation<'a>> + 'a {
        applied
            .iter()
            .filter_map(move |applied_op| {
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
        operations: &'a HashMap<OperationHash, Operation>,
        protocol_hash: &'a str,
    ) -> impl Iterator<Item = MonitoredOperation<'a>> + 'a {
        errored
            .iter()
            .filter_map(move |errored_op| {
                let op_hash = errored_op.hash.to_base58_check();
                let operation = operations.get(&errored_op.hash)?;
                let json = &errored_op.protocol_data_json_with_error_json.protocol_data_json;
                let (protocol_data, err) = match serde_json::from_str(json) {
                    Ok(protocol_data) => (protocol_data, None),
                    Err(err) => (HashMap::default(), Some(err.to_string())),
                };
                let ocaml_err = &errored_op
                    .protocol_data_json_with_error_json
                    .error_json;
                Some(MonitoredOperation {
                    branch: operation.branch().to_base58_check(),
                    protocol: protocol_hash,
                    hash: op_hash,
                    protocol_data,
                    error: serde_json::from_str(ocaml_err)
                        .unwrap_or_else(|err| vec![err.to_string()]),
                    protocol_data_parse_error: err,
                })
            })
    }
}
