use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock}, time::Duration,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::node::Node;

pub type MempoolEndorsementStats = BTreeMap<String, OperationStats>;

#[derive(Debug, Error)]
pub enum StatisticMonitorError {
    /// Storage error
    #[error("Error while writing into storage, reason: {reason}")]
    StorageError { reason: String },

    #[error("Error in node RPC, reason: {0}")]
    NodeRpcError(#[from] reqwest::Error),
}

pub struct EndorsementSummaryStorage {
    inner: Arc<RwLock<BTreeMap<i32, EndorsementOperationSummary>>>,
}

impl EndorsementSummaryStorage {
    pub fn new() -> Self {
        EndorsementSummaryStorage {
            inner: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    pub fn insert(
        &mut self,
        level: i32,
        summary: EndorsementOperationSummary,
    ) -> Result<(), StatisticMonitorError> {
        self.inner.write().map(|mut write_locked_storage| {
            write_locked_storage.insert(level, summary);
        }).map_err(|e| StatisticMonitorError::StorageError { reason: e.to_string() })
    }

    pub fn get(&self, level: i32) -> Result<Option<EndorsementOperationSummary>, StatisticMonitorError> {
        self.inner.read().map(|read_locked_storage| {
            read_locked_storage.get(&level).cloned()
        }).map_err(|e| StatisticMonitorError::StorageError { reason: e.to_string() })
    }
}

pub struct StatisticsMonitor {
    pub node: Node,
    pub endorsmenet_summary_storage: EndorsementSummaryStorage,
    // pub delegate: String,
}

impl StatisticsMonitor {
    pub fn new(node: Node, /*delegate: &str*/) -> Self {
        Self {
            node,
            endorsmenet_summary_storage: EndorsementSummaryStorage::new(),
            // delegate: delegate.to_string(),
        }
    }

    pub async fn run(&mut self) -> Result<(), StatisticMonitorError> {
        loop {
            let current_head = self.node.get_head_data().await.unwrap();
            let mempool_endorsements = self.get_memopool_endorsement_stats().await?;

            // TODO: more injected endorsements? should not happen?
            // let injected_endorsement = mempool_endorsements.values().filter(|op| op.is_injected()).last();

            // TOOD: remove, just for testing purposes
            let injected_endorsement = mempool_endorsements.values().last();

            if let Some(stats) = injected_endorsement {
                let endorsement_summary = 
                    EndorsementOperationSummary::new(
                        OffsetDateTime::parse(current_head.timestamp(),
                        &Rfc3339).unwrap(), stats.clone()
                    );
                self.endorsmenet_summary_storage.insert(*current_head.level() as i32, endorsement_summary)?;
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        // Ok(())
    }

    async fn get_memopool_endorsement_stats(&self) -> Result<MempoolEndorsementStats, reqwest::Error> {
        reqwest::get(&format!(
            "http://127.0.0.1:{}/dev/shell/automaton/stats/mempool/endorsements",
            self.node.port()
        ))
        .await?
        .json()
        .await
    }
}

impl EndorsementOperationSummary {
    pub fn new(
        current_head_timestamp: OffsetDateTime,
        op_stats: OperationStats,
        // block_stats: Option<BlockApplicationStatistics>,
    ) -> Self {
        // let block_received = block_stats.clone().map(|stats| {
        //     let current_head_nanos = current_head_timestamp.unix_timestamp_nanos();
        //     ((stats.receive_timestamp as i128) - current_head_nanos) as i64
        // });

        // let block_application = block_stats.clone().and_then(|stats| {
        //     stats
        //         .apply_block_end
        //         .and_then(|end| stats.apply_block_start.map(|start| (end - start) as i64))
        // });

        // let injected = op_stats.injected_timestamp.and_then(|inject_time| {
        //     block_stats.map(|stats| (inject_time as i64) - stats.receive_timestamp)
        // });

        let validated = op_stats.validation_duration();

        let operation_hash_sent = op_stats
            .first_sent()
            .and_then(|sent| op_stats.validation_ended().map(|v_end| sent - v_end));

        let operation_requested = op_stats
            .first_content_requested_remote()
            .and_then(|op_req| op_stats.first_sent().map(|sent| op_req - sent));

        let operation_sent = op_stats.first_content_sent().and_then(|cont_sent| {
            op_stats
                .first_content_requested_remote()
                .map(|op_req| cont_sent - op_req)
        });

        Self {
            // TODO: once we get application statistics, remove defaults
            block_received: Default::default(),
            block_application: Default::default(),
            injected: Default::default(),
            validated,
            operation_hash_sent,
            operation_requested,
            operation_sent,
            operation_hash_received_back: None, // TODO
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EndorsementOperationSummary {
    pub block_application: Option<i64>,
    pub block_received: Option<i64>,
    pub injected: Option<i64>,
    pub validated: Option<i64>,
    pub operation_hash_sent: Option<i64>,
    pub operation_requested: Option<i64>,
    pub operation_sent: Option<i64>,
    pub operation_hash_received_back: Option<u64>,
}

#[derive(Deserialize, Clone, Debug, Default, Serialize, PartialEq)]
#[allow(dead_code)] // TODO: make BE send only the relevant data
pub struct OperationStats {
    kind: Option<OperationKind>,
    /// Minimum time when we saw this operation. Latencies are measured
    /// from this point.
    min_time: Option<u64>,
    first_block_timestamp: Option<u64>,
    validation_started: Option<i64>,
    /// (time_validation_finished, validation_result, prevalidation_duration)
    validation_result: Option<(i64, OperationValidationResult, Option<i64>, Option<i64>)>,
    validations: Vec<OperationValidationStats>,
    nodes: BTreeMap<String, OperationNodeStats>,
    pub injected_timestamp: Option<u64>,
}

#[derive(Deserialize, Clone, Debug, Serialize, PartialEq)]
#[allow(dead_code)] // TODO: make BE send only the relevant data
pub struct OperationNodeStats {
    received: Vec<OperationNodeCurrentHeadStats>,
    sent: Vec<OperationNodeCurrentHeadStats>,

    content_requested: Vec<i64>,
    content_received: Vec<i64>,

    content_requested_remote: Vec<i64>,
    content_sent: Vec<i64>,
}

#[derive(Deserialize, Debug, Clone, Default, Serialize, PartialEq)]
#[allow(dead_code)] // TODO: make BE send only the relevant data
pub struct OperationNodeCurrentHeadStats {
    /// Latency from first time we have seen that operation.
    latency: i64,
    block_level: i32,
    block_timestamp: i64,
}

#[derive(Deserialize, Debug, Clone, Serialize, PartialEq)]
#[allow(dead_code)] // TODO: make BE send only the relevant data
pub struct OperationValidationStats {
    started: Option<i64>,
    finished: Option<i64>,
    preapply_started: Option<i64>,
    preapply_ended: Option<i64>,
    current_head_level: Option<i32>,
    result: Option<OperationValidationResult>,
}

#[derive(
    Deserialize,
    Debug,
    Clone,
    Copy,
    strum_macros::Display,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
)]
pub enum OperationKind {
    Endorsement,
    SeedNonceRevelation,
    DoubleEndorsement,
    DoubleBaking,
    Activation,
    Proposals,
    Ballot,
    EndorsementWithSlot,
    FailingNoop,
    Reveal,
    Transaction,
    Origination,
    Delegation,
    RegisterConstant,
    Unknown,
    Default,
}

impl Default for OperationKind {
    fn default() -> Self {
        OperationKind::Default
    }
}

#[derive(Deserialize, Debug, Clone, Copy, strum_macros::Display, Serialize, PartialEq)]
pub enum OperationValidationResult {
    Applied,
    Refused,
    BranchRefused,
    BranchDelayed,
    Prechecked,
    PrecheckRefused,
    Prevalidate,
    Default,
}

impl OperationStats {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_injected(&self) -> bool {
        self.injected_timestamp.is_some()
    }

    pub fn validation_duration(&self) -> Option<i64> {
        self.validation_started
            .and_then(|start| self.validation_result.map(|(end, _, _, _)| end - start))
    }

    pub fn validation_ended(&self) -> Option<i64> {
        self.validation_result.map(|(end, _, _, _)| end)
    }

    pub fn first_sent(&self) -> Option<i64> {
        self.nodes
            .clone()
            .into_iter()
            .filter_map(|(_, v)| {
                v.sent
                    .into_iter()
                    .min_by_key(|v| v.latency)
                    .map(|v| v.latency)
            })
            .min()
    }

    pub fn first_content_requested_remote(&self) -> Option<i64> {
        self.nodes
            .clone()
            .into_iter()
            .filter_map(|(_, v)| v.content_requested_remote.into_iter().min())
            .min()
    }

    pub fn first_content_sent(&self) -> Option<i64> {
        self.nodes
            .clone()
            .into_iter()
            .filter_map(|(_, v)| v.content_sent.into_iter().min())
            .min()
    }
}