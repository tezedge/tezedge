// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
use std::convert::TryFrom;
use std::pin::Pin;

use futures::task::{Context, Poll};
use futures::Stream;
use serde::Serialize;
use slog::{error, Logger};

use crypto::hash::{BlockHash, ProtocolHash};
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
            fitness: block.header.fitness().as_hex_vec(),
            context: block.header.context().to_base58_check(),
            protocol_data: hex::encode(block.header.protocol_data()),
        })
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

// TODO: add tests for Stream!
