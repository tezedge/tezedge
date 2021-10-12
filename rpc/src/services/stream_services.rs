// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::pin::Pin;

use futures::task::{Context, Poll};
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slog::{error, Logger};
use tezos_api::ffi::{Applied, Errored};
use tezos_messages::p2p::encoding::operation::Operation;

use crypto::hash::{BlockHash, ChainId, OperationHash, ProtocolHash};
use shell::mempool::CurrentMempoolStateStorageRef;
use shell_integration::{generate_stream_id, StreamCounter, StreamId};
use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockMetaStorageReader, PersistentStorage};
use tezos_messages::{ts_to_rfc3339, TimestampOutOfRangeError};

use crate::helpers::RpcServiceError;
use crate::server::RpcCollectedStateRef;

/// Object containing information to recreate the block header shell information
#[derive(Serialize, Debug, Clone)]
struct BlockHeaderMonitorInfo {
    pub hash: String,
    pub level: i32,
    pub proto: u8,
    pub predecessor: String,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: String,
    pub fitness: Vec<String>,
    pub context: String,
    pub protocol_data: String,
}

impl TryFrom<&BlockHeaderWithHash> for BlockHeaderMonitorInfo {
    type Error = TimestampOutOfRangeError;

    fn try_from(block: &BlockHeaderWithHash) -> Result<Self, Self::Error> {
        Ok(BlockHeaderMonitorInfo {
            hash: block.hash.to_base58_check(),
            level: block.header.level(),
            proto: block.header.proto(),
            predecessor: block.header.predecessor().to_base58_check(),
            timestamp: ts_to_rfc3339(block.header.timestamp())?,
            validation_pass: block.header.validation_pass(),
            operations_hash: block.header.operations_hash().to_base58_check(),
            fitness: block
                .header
                .fitness()
                .iter()
                .map(|x| hex::encode(&x))
                .collect(),
            context: block.header.context().to_base58_check(),
            protocol_data: hex::encode(block.header.protocol_data()),
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct MempoolOperationsQuery {
    pub applied: bool,
    pub refused: bool,
    pub branch_delayed: bool,
    pub branch_refused: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitoredOperation {
    branch: String,

    #[serde(flatten)]
    protocol_data: HashMap<String, Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
    #[serde(skip_serializing)]
    _hash: String,
    #[serde(skip_serializing)]
    error: Option<String>,
}

impl MonitoredOperation {
    pub fn collect_applied(
        applied: &[Applied],
        operations: &HashMap<OperationHash, Operation>,
        protocol_hash: &str,
        streamed_operations: &mut HashSet<String>,
    ) -> Result<Vec<Self>, RpcServiceError> {
        let mut result = Vec::with_capacity(applied.len());
        for applied_op in applied {
            let op_hash = applied_op.hash.to_base58_check();

            // if the operation is allready included in the stream, ignore it
            if streamed_operations.contains(&op_hash) {
                continue;
            }

            streamed_operations.insert(op_hash.clone());

            let operation = match operations.get(&applied_op.hash) {
                Some(op) => op,
                None => {
                    return Err(RpcServiceError::NoDataFoundError {
                        reason: "Operation hash not found in operations data".into(),
                    })
                }
            };
            let monitored_op = MonitoredOperation {
                branch: operation.branch().to_base58_check(),
                protocol: Some(protocol_hash.to_string()),
                hash: op_hash,
                protocol_data: serde_json::from_str(&applied_op.protocol_data_json)?,
                error: None,
            };
            result.push(monitored_op)
        }
        Ok(result)
    }

    pub fn collect_errored(
        errored: &[Errored],
        operations: &HashMap<OperationHash, Operation>,
        protocol_hash: &str,
        streamed_operations: &mut HashSet<String>,
    ) -> Result<Vec<Self>, RpcServiceError> {
        let mut result = Vec::with_capacity(errored.len());
        for errored_op in errored {
            let op_hash = errored_op.hash.to_base58_check();

            // if the operation is allready included in the stream, ignore it
            if streamed_operations.contains(&op_hash) {
                continue;
            }

            streamed_operations.insert(op_hash.clone());

            let operation = match operations.get(&errored_op.hash) {
                Some(op) => op,
                None => {
                    return Err(RpcServiceError::NoDataFoundError {
                        reason: "Operation hash not found in operations data".into(),
                    })
                }
            };
            let monitored_op = MonitoredOperation {
                branch: operation.branch().to_base58_check(),
                protocol: Some(protocol_hash.to_string()),
                hash: op_hash,
                protocol_data: serde_json::from_str(
                    &errored_op
                        .protocol_data_json_with_error_json
                        .protocol_data_json,
                )?,
                error: Some(
                    errored_op
                        .protocol_data_json_with_error_json
                        .error_json
                        .clone(),
                ),
            };
            result.push(monitored_op)
        }
        Ok(result)
    }
}

pub struct HeadMonitorStream {
    block_meta_storage: BlockMetaStorage,

    state: RpcCollectedStateRef,
    last_checked_head: Option<BlockHash>,
    protocol: Option<ProtocolHash>,
    contains_waker: bool,
    stream_id: StreamId,
    log: Logger,
}

pub struct OperationMonitorStream {
    _chain_id: ChainId,
    current_mempool_state_storage: CurrentMempoolStateStorageRef,
    state: RpcCollectedStateRef,
    last_checked_head: BlockHash,
    log: Logger,
    contains_waker: bool,
    stream_id: StreamId,
    streamed_operations: HashSet<String>,
    query: MempoolOperationsQuery,
    poll_counter: usize,
}

impl OperationMonitorStream {
    pub fn new(
        _chain_id: ChainId,
        current_mempool_state_storage: CurrentMempoolStateStorageRef,
        state: RpcCollectedStateRef,
        log: Logger,
        last_checked_head: BlockHash,
        mempool_operaions_query: MempoolOperationsQuery,
    ) -> Self {
        Self {
            _chain_id,
            current_mempool_state_storage,
            state,
            last_checked_head,
            log,
            contains_waker: false,
            query: mempool_operaions_query,
            streamed_operations: HashSet::new(),
            stream_id: generate_stream_id(),
            poll_counter: 0,
        }
    }

    fn yield_operations(&mut self) -> Poll<Option<Result<String, RpcServiceError>>> {
        let OperationMonitorStream {
            current_mempool_state_storage,
            query,
            streamed_operations,
            ..
        } = self;

        // 1. get the operations currently in mempool
        match current_mempool_state_storage.read() {
            Ok(current_mempool_state) => {
                let (validate_operation_result, operations, protocol_hash) =
                    match current_mempool_state.prevalidator() {
                        Some(prevalidator) => (
                            current_mempool_state.result(),
                            current_mempool_state.operations(),
                            prevalidator.protocol.to_base58_check(),
                        ),
                        None => {
                            // No prevalidator present means the node is not bootstrapped and cannot process mempool operations
                            // end the stream in this case
                            return Poll::Ready(None);
                        }
                    };
                let mut requested_ops =
                    Vec::with_capacity(validate_operation_result.operations_count());

                // 2. collect the requested operations
                if query.applied {
                    let monitored_applied = MonitoredOperation::collect_applied(
                        &validate_operation_result.applied,
                        operations,
                        &protocol_hash,
                        streamed_operations,
                    )?;
                    requested_ops.extend(monitored_applied);
                }
                if query.refused {
                    let monitored_refused = MonitoredOperation::collect_errored(
                        &validate_operation_result.refused,
                        operations,
                        &protocol_hash,
                        streamed_operations,
                    )?;
                    requested_ops.extend(monitored_refused);
                }
                if query.branch_delayed {
                    let monitored_branch_delayed = MonitoredOperation::collect_errored(
                        &validate_operation_result.branch_delayed,
                        operations,
                        &protocol_hash,
                        streamed_operations,
                    )?;
                    requested_ops.extend(monitored_branch_delayed);
                }
                if query.branch_refused {
                    let monitored_branch_refused = MonitoredOperation::collect_errored(
                        &validate_operation_result.branch_refused,
                        operations,
                        &protocol_hash,
                        streamed_operations,
                    )?;
                    requested_ops.extend(monitored_branch_refused);
                }

                // handle special case, when the first poll has no operations, return an empty vector
                if requested_ops.is_empty() && self.poll_counter == 1 {
                    return Poll::Ready(Some(Ok("[]\n".to_string())));
                } else if requested_ops.is_empty() {
                    return Poll::Pending;
                }

                let mut to_yield_string: String = serde_json::to_string(&requested_ops)?;
                to_yield_string = to_yield_string.replace("\\", "");
                to_yield_string.push('\n');
                Poll::Ready(Some(Ok(to_yield_string)))
            }
            Err(_) => Poll::Ready(None),
        }
    }
}

impl HeadMonitorStream {
    pub fn new(
        state: RpcCollectedStateRef,
        protocol: Option<ProtocolHash>,
        persistent_storage: &PersistentStorage,
        log: Logger,
    ) -> Self {
        let stream_id = generate_stream_id();
        Self {
            state,
            protocol,
            last_checked_head: None,
            block_meta_storage: BlockMetaStorage::new(persistent_storage),
            contains_waker: false,
            stream_id,
            log,
        }
    }

    fn yield_head(
        &self,
        current_head: &BlockHeaderWithHash,
    ) -> Result<Option<String>, RpcServiceError> {
        let HeadMonitorStream { protocol, .. } = self;

        if let Some(protocol) = &protocol {
            let block_additional_data = match self
                .block_meta_storage
                .get_additional_data(&current_head.hash)?
            {
                Some(block_additional_data) => block_additional_data,
                None => {
                    return Err(RpcServiceError::NoDataFoundError {
                        reason: format!(
                            "Missing block additional data for block_hash: {}",
                            current_head.hash.to_base58_check()
                        ),
                    })
                }
            };

            if &block_additional_data.next_protocol_hash != protocol {
                return Ok(None);
            }
        }

        // serialize the struct to a json string to yield by the stream
        let mut head_string =
            serde_json::to_string(&BlockHeaderMonitorInfo::try_from(current_head)?)?;

        // push a newline character to the stream
        head_string.push('\n');

        Ok(Some(head_string))
    }
}

impl Stream for HeadMonitorStream {
    type Item = Result<String, RpcServiceError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<String, RpcServiceError>>> {
        // Note: the stream only ends on the client dropping the connection or on error

        let mut rpc_state = match self.state.write() {
            Ok(state) => state,
            Err(e) => {
                error!(
                    self.log,
                    "HeadMonitorStream cannot access shared rpc state, reason: {}", e
                );
                return Poll::Ready(None);
            }
        };

        let current_head = rpc_state.current_head().clone();

        if !self.contains_waker {
            rpc_state.add_stream(self.stream_id, cx.waker().clone());

            // drop the immutable borrow so we can borrow self again as mutable
            drop(rpc_state);

            self.contains_waker = true;
        } else {
            // drop the immutable borrow so we can borrow self again as mutable
            drop(rpc_state);
        }

        // if last_checked_head is None, this is the first poll, yield the current_head
        let last_checked_head = if let Some(head_hash) = &self.last_checked_head {
            head_hash
        } else {
            // first poll
            self.last_checked_head = Some(current_head.hash.clone());
            // If there is no head with the desired protocol, [yield_head] returns Ok(None) which is transposed to None, meaning we
            // would end the stream, in this case, we need to Pend.
            if let Some(head_string_result) = self.yield_head(&current_head).transpose() {
                return Poll::Ready(Some(head_string_result));
            } else {
                return Poll::Pending;
            };
        };

        if last_checked_head == &current_head.hash {
            // current head not changed, yield nothing
            Poll::Pending
        } else {
            // Head change, yield new head
            self.last_checked_head = Some(current_head.hash.clone());
            // If there is no head with the desired protocol, [yield_head] returns Ok(None) which is transposed to None, meaning we
            // would end the stream, in this case, we need to Pend.
            if let Some(head_string_result) = self.yield_head(&current_head).transpose() {
                Poll::Ready(Some(head_string_result))
            } else {
                Poll::Pending
            }
        }
    }
}

// The head stream is only closed by the client, remove the stream from the map when the stream goes out of scope
impl Drop for HeadMonitorStream {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.write() {
            state.remove_stream(self.stream_id);
        };
    }
}

impl Stream for OperationMonitorStream {
    type Item = Result<String, RpcServiceError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<String, RpcServiceError>>> {
        self.poll_counter += 1;

        if !self.contains_waker {
            let mut mempool_state = match self.current_mempool_state_storage.write() {
                Ok(state) => state,
                Err(e) => {
                    error!(
                        self.log,
                        "OperationMonitorStream cannot access shared mempool state, reason: {}", e
                    );
                    return Poll::Ready(None);
                }
            };
            mempool_state.add_stream(self.stream_id, cx.waker().clone());
            drop(mempool_state);
            self.contains_waker = true;
        }

        let state = match self.state.read() {
            Ok(state) => state,
            Err(e) => {
                error!(
                    self.log,
                    "OperationMonitorStream cannot access shared rpc state, reason: {}", e
                );
                return Poll::Ready(None);
            }
        };
        let current_head = state.current_head().clone();

        // drop the immutable borrow so we can borrow self again as mutable
        drop(state);

        if self.last_checked_head == current_head.hash {
            // current head not changed, check for new operations
            let yielded = self.yield_operations();
            match yielded {
                Poll::Pending => Poll::Pending,
                _ => yielded,
            }
        } else {
            // Head change, end stream
            let mut mempool_state = match self.current_mempool_state_storage.write() {
                Ok(state) => state,
                Err(e) => {
                    error!(
                        self.log,
                        "OperationMonitorStream cannot access shared mempool state, reason: {}", e
                    );
                    return Poll::Ready(None);
                }
            };
            mempool_state.remove_stream(self.stream_id);
            Poll::Ready(None)
        }
    }
}

impl Drop for OperationMonitorStream {
    fn drop(&mut self) {
        if let Ok(mut state) = self.current_mempool_state_storage.write() {
            state.remove_stream(self.stream_id);
        };
    }
}

// TODO: add tests for both Streams!
