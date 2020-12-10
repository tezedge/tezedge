// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
use std::{collections::{HashMap, HashSet}};
use std::pin::Pin;

use futures::Stream;
use futures::task::{Context, Poll};
use std::future::Future;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slog::{Logger, warn};
use tokio::time::{Delay, delay_until};
use tokio::time::{Duration, Instant};

use crypto::hash::{BlockHash, chain_id_to_b58_string, ChainId, HashType, ProtocolHash};
use shell::shell_channel::BlockApplied;

use crate::rpc_actor::RpcCollectedStateRef;
use crate::server::RpcServiceEnvironment;
use crate::services::mempool_services::get_pending_operations;
use crate::helpers::{BlockHeaderInfo, FullBlockInfo};

pub const MONITOR_TIMER_MILIS: u64 = 500;

/// Object containing information to recreate the block header shell information
#[derive(Serialize, Debug, Clone)]
pub struct BlockHeaderMonitorInfo {
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

#[derive(Copy, Clone, Debug)]
pub struct MempoolOperationsQuery {
    pub applied: bool,
    pub refused: bool,
    pub branch_delayed: bool,
    pub branch_refused: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonitoredOperation {
    pub signature: String,
    pub branch: String,
    pub contents: Value,

    #[serde(skip_deserializing)]
    pub protocol: Option<String>,
    #[serde(skip_serializing)]
    pub hash: String,
    #[serde(skip_serializing)]
    pub error: Option<Value>,
}

pub struct HeadMonitorStream {
    pub chain_id: ChainId,
    pub state: RpcCollectedStateRef,
    pub last_checked_head: Option<BlockHash>,
    pub log: Logger,
    pub delay: Option<Delay>,
    pub protocol: Option<ProtocolHash>,
}

pub struct OperationMonitorStream {
    pub chain_id: ChainId,
    pub state: RpcCollectedStateRef,
    pub last_checked_head: Option<BlockHash>,
    pub log: Logger,
    pub delay: Option<Delay>,
    pub streamed_operations: Option<HashSet<String>>,
    pub query: Option<MempoolOperationsQuery>,
}

impl OperationMonitorStream {
    pub fn new(chain_id: &ChainId, env: &RpcServiceEnvironment, mempool_operaions_query: Option<MempoolOperationsQuery>) -> Self {
        Self {
            chain_id: chain_id.clone(),
            state: env.state().clone(),
            last_checked_head: Some(env.state().read().unwrap().current_head().as_ref().unwrap().header().hash.clone()),
            log: env.log().clone(),
            delay: None,
            query: mempool_operaions_query,
            streamed_operations: None,
        }
    }

    fn yield_operations(&mut self) -> Poll<Option<Result<String, failure::Error>>> {
        let (pending_operations, protocol) = if let Ok(ops) = get_pending_operations(&self.chain_id, &self.state) {
            ops
        } else {
            return Poll::Ready(None)
        };
        let mut requested_ops: HashMap<String, Value> = HashMap::new();

        // no querry means all ops
        let query = if let Some(q) = self.query {
            q
        } else {
            MempoolOperationsQuery{
                applied: true,
                refused: true,
                branch_delayed: true,
                branch_refused :true,
            }
        };

        // fill in the resulting vector according to the querry
        if query.applied {
            let applied: HashMap<_, _> = pending_operations.applied.into_iter()
                .map(|v| (v["hash"].to_string(), serde_json::to_value(v).unwrap()))
                .collect();
            requested_ops.extend(applied);
        }
        if query.branch_delayed {
            let branch_delayed: HashMap<_, _> = pending_operations.branch_delayed.into_iter()
                .map(|v| (v["hash"].to_string(), v))
                .collect();
            requested_ops.extend(branch_delayed);
        }
        if query.branch_refused {
            let branch_refused: HashMap<_, _> = pending_operations.branch_refused.into_iter()
                .map(|v| (v["hash"].to_string(), v))
                .collect();
            requested_ops.extend(branch_refused);
        }
        if query.refused {
            let refused: HashMap<_, _> = pending_operations.refused.into_iter()
                .map(|v| (v["hash"].to_string(), v))
                .collect();
            requested_ops.extend(refused);
        }

        if let Some(streamed_operations) = &mut self.streamed_operations {
            let to_yield: Vec<MonitoredOperation> = requested_ops.clone().into_iter()
                .filter(|(k, _)| !streamed_operations.contains(k))
                .map(|(_, v)| {
                    let mut monitor_op: MonitoredOperation = serde_json::from_value(v).unwrap();
                    monitor_op.protocol = Some(HashType::ProtocolHash.hash_to_b58check(&protocol));
                    monitor_op
                })
                .collect();

            for op_hash in requested_ops.keys() {
                streamed_operations.insert(op_hash.to_string());
            }

            if to_yield.is_empty() {
                Poll::Pending
            } else {
                let mut to_yield_string = serde_json::to_string(&to_yield)?;
                to_yield_string = to_yield_string.replace("\\", "");
                to_yield_string.push('\n');
                Poll::Ready(Some(Ok(to_yield_string)))
            }
        } else {
            let mut streamed_operations = HashSet::<String>::new();
            let to_yield: Vec<MonitoredOperation> = requested_ops.into_iter()
                .map(|(k, v)| {
                    streamed_operations.insert(k);
                    let mut monitor_op: MonitoredOperation = match serde_json::from_value(v) {
                        Ok(json_value) => {
                            json_value
                        }
                        Err(e) => {
                            warn!(self.log, "Wont yield errored op: {}", e);
                            return Err(e)
                        }
                    };
                    monitor_op.protocol = Some(HashType::ProtocolHash.hash_to_b58check(&protocol));
                    Ok(monitor_op)
                })
                .filter_map(Result::ok)
                .collect();

                
            self.streamed_operations = Some(streamed_operations);
            let mut to_yield_string = serde_json::to_string(&to_yield)?;
            to_yield_string = to_yield_string.replace("\\", "");
            to_yield_string.push('\n');
            Poll::Ready(Some(Ok(to_yield_string)))
        }
    }
}

impl HeadMonitorStream {
    pub fn new(chain_id: &ChainId, env: &RpcServiceEnvironment, protocol: Option<ProtocolHash>) -> Self {
        Self {
            chain_id: chain_id.clone(),
            state: env.state().clone(),
            last_checked_head: None,
            log: env.log().clone(),
            delay: None,
            protocol,
        }
    }

    fn yield_head(&self, current_head: Option<BlockApplied>) -> Result<Option<String>, failure::Error> {
        // get the desired structure of the
        let current_head_header = current_head.as_ref().map(|current_head| {
            let chain_id = chain_id_to_b58_string(&self.chain_id);
            BlockHeaderInfo::new(current_head, chain_id).to_monitor_header(current_head)
        });

        if let Some(protocol) = &self.protocol {
            let block_info = current_head.as_ref().map(|current_head| {
                let chain_id = chain_id_to_b58_string(&self.chain_id);
                FullBlockInfo::new(current_head, chain_id)
            });
            let block_next_protocol = if let Some(block_info) = block_info {
                block_info.metadata["next_protocol"].to_string().replace("\"", "")
            } else {
                return Ok(None)
            };
            if &HashType::ProtocolHash.b58check_to_hash(&block_next_protocol)? != protocol {
                return Ok(None)
            }
        }

        // serialize the struct to a json string to yield by the stream
        let mut head_string = serde_json::to_string(&current_head_header.unwrap())?;

        // push a newline character to the stream to imrove readability
        head_string.push('\n');

        Ok(Some(head_string))
    }
}


impl Stream for HeadMonitorStream {
    type Item = Result<String, failure::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<String, failure::Error>>> {
        // Note: the stream only ends on the client dropping the connection

        // create or get a delay future, that blocks for MONITOR_TIMER_MILIS
        let delay = self.delay.get_or_insert_with(|| {
            let when = Instant::now() + Duration::from_millis(MONITOR_TIMER_MILIS);
            delay_until(when)
        });

        // pin the future pointer
        let mut pinned = std::boxed::Box::pin(delay);
        let pinned_mut = pinned.as_mut();

        // poll the delay future
        match pinned_mut.poll(cx) {
            Poll::Pending => {
                Poll::Pending
            },
            _ => {
                // get rid of the used delay
                self.delay = None;

                let state = self.state.read().unwrap();
                let current_head = state.current_head().clone();

                // drop the immutable borrow so we can borrow self again as mutable
                // TODO: refactor this drop (remove if possible)
                drop(state);

                // if last_checked_head is none, this is the first poll, yield the current_head
                let last_checked_head = if let Some(head_hash) = &self.last_checked_head {
                    head_hash
                } else {
                    // first poll
                    if let Some(current_head) = current_head {
                        self.last_checked_head = Some(current_head.header().hash.clone());
                        let head_string_result = self.yield_head(Some(current_head));
                        return Poll::Ready(head_string_result.transpose())
                    } else {
                        // No current head found, storage not ready yet
                        return Poll::Ready(None)
                    }
                };

                if let Some(current_head) = current_head {
                    if last_checked_head == &current_head.header().hash {
                        // current head not changed, yield nothing
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    } else {
                        // Head change, yield new head
                        self.last_checked_head = Some(current_head.header().hash.clone());
                        let head_string_result = self.yield_head(Some(current_head));
                        Poll::Ready(head_string_result.transpose())
                    }
                } else {
                    // No current head found, storage not ready yet
                    Poll::Ready(None)
                }
            }
        }
    }
}

impl Stream for OperationMonitorStream {
    type Item = Result<String, failure::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<String, failure::Error>>> {
        // create or get a delay future, that blocks for MONITOR_TIMER_MILIS
        let delay = self.delay.get_or_insert_with(|| {
            let when = Instant::now() + Duration::from_millis(MONITOR_TIMER_MILIS);
            delay_until(when)
        });

        // pin the future pointer
        let mut pinned = std::boxed::Box::pin(delay);
        let pinned_mut = pinned.as_mut();

        // poll the delay future
        match pinned_mut.poll(cx) {
            Poll::Pending => {
                Poll::Pending
            },
            _ => {
                // get rid of the used delay
                self.delay = None;

                let state = self.state.read().unwrap();
                let current_head = state.current_head().clone();

                // drop the immutable borrow so we can borrow self again as mutable
                // TODO: refactor this drop (remove if possible)
                drop(state);

                let last_checked_head = if let Some(head_hash) = &self.last_checked_head {
                    head_hash
                } else {
                    return Poll::Ready(None)
                };

                if let Some(current_head) = current_head {
                    if last_checked_head == &current_head.header().hash {
                        // current head not changed, check for new operations
                        let yielded = self.yield_operations();
                        match yielded {
                            Poll::Pending => {
                                cx.waker().wake_by_ref();
                                Poll::Pending
                            },
                            _ => {
                                yielded
                            },
                        }
                    } else {
                        // Head change, end stream
                        Poll::Ready(None)
                    }
                } else {
                    // No current head found, storage not ready yet 
                    Poll::Ready(None)
                }
            }
        }
    }
}
