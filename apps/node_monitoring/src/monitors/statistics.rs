use std::{
    collections::BTreeMap,
    fmt::Display,
    sync::{Arc, RwLock},
    time::Duration,
};

use itertools::Itertools;
use num::ToPrimitive;
use serde::{Deserialize, Serialize};
use slog::{debug, Logger};
use strum_macros::EnumIter;
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::node::Node;

use super::delegate::{DelegateEndorsingRights, EndorsingRights};

pub type MempoolEndorsementStats = BTreeMap<String, OperationStats>;
pub type PreendorsementStatus = EndorsementStatus;

#[derive(Debug, Error)]
pub enum StatisticMonitorError {
    /// Storage error
    #[error("Error while writing into storage, reason: {reason}")]
    StorageError { reason: String },

    #[error("Error in node RPC, reason: {0}")]
    NodeRpcError(#[from] reqwest::Error),
}

#[derive(Clone, Debug)]
pub struct LockedBTreeMap<K: Ord, V: Clone> {
    inner: Arc<RwLock<BTreeMap<K, V>>>,
}

impl<K: Ord, V: Clone> LockedBTreeMap<K, V> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Result<(), StatisticMonitorError> {
        self.inner
            .write()
            .map(|mut write_locked_storage| {
                write_locked_storage.insert(key, value);
            })
            .map_err(|e| StatisticMonitorError::StorageError {
                reason: e.to_string(),
            })
    }

    pub fn get(&self, key: K) -> Result<Option<V>, StatisticMonitorError> {
        self.inner
            .read()
            .map(|read_locked_storage| read_locked_storage.get(&key).cloned())
            .map_err(|e| StatisticMonitorError::StorageError {
                reason: e.to_string(),
            })
    }
}

pub struct StatisticsMonitor {
    pub node: Node,
    pub endorsmenet_summary_storage: LockedBTreeMap<i32, EndorsementOperationSummary>,
    pub application_statistics_storage: LockedBTreeMap<String, BlockApplicationStatistics>,
    pub delegates: Vec<String>,
    pub last_seen_current_head: Option<String>,
    log: Logger,
}

impl StatisticsMonitor {
    pub fn new(
        node: Node,
        delegates: Vec<String>,
        endorsmenet_summary_storage: LockedBTreeMap<i32, EndorsementOperationSummary>,
        log: Logger,
    ) -> Self {
        Self {
            node,
            endorsmenet_summary_storage,
            application_statistics_storage: LockedBTreeMap::new(),
            delegates,
            last_seen_current_head: None,
            log,
        }
    }

    pub async fn run(&mut self) -> Result<(), StatisticMonitorError> {
        loop {
            let current_head = self.node.get_head_data().await.unwrap();
            let current_head_level = *current_head.level() as i32;
            let current_head_round = *current_head.payload_round() as i32;
            let current_head_hash = current_head.block_hash();

            let constants = self.get_network_constants().await?;

            debug!(self.log, "Current head - Level: {current_head_level} Round: {current_head_round} Hash: {current_head_hash}");

            // TODO: this should occure only on head change (?)
            let application_statistics = self.get_application_stats(current_head_level).await?;
            for stats in application_statistics {
                self.application_statistics_storage
                    .insert(stats.block_hash.clone(), stats)?;
            }

            let mempool_endorsements = self.get_memopool_endorsement_stats().await?;
            // let endorsements_status = ;

            let block_application_stats = self
                .application_statistics_storage
                .get(current_head.block_hash().to_string())?;

            for delegate in &self.delegates {
                if let Some(delegate_rigths) = self
                    .get_endorsing_rights_for_delegate(current_head_level, delegate)
                    .await?
                {
                    let preendorsmenent_statuses = self
                        .get_preendorsement_statuses(current_head_level, current_head_round)
                        .await?;
                    let endorsmenent_statuses = self
                        .get_endorsement_statuses(current_head_level, current_head_round)
                        .await?;
                    let delegate_slot = delegate_rigths.delegates[0].get_first_slot();
                    // let injected_endorsement = mempool_endorsements.values().filter(|op| op.is_injected());
                    if let Some(injected_endorsement_op_hash) = endorsmenent_statuses
                        .iter()
                        .filter(|(_, endorsement)| endorsement.slot == delegate_slot)
                        .map(|(op_h, _)| op_h)
                        .last()
                    {
                        let injected_preendorsement_op_hash = preendorsmenent_statuses
                            .iter()
                            .filter(|(_, preendorsement)| preendorsement.slot == delegate_slot)
                            .map(|(op_h, _)| op_h)
                            .last()
                            .unwrap();

                        // println!("Preendorsement op hash: {injected_preendorsement_op_hash}");
                        // println!("Endorsement op hash: {injected_endorsement_op_hash}");

                        let injected_preendorsement =
                            mempool_endorsements.get(injected_preendorsement_op_hash);
                        let injected_endorsement =
                            mempool_endorsements.get(injected_endorsement_op_hash);

                        // println!("Preendorsement op: {:#?}", injected_preendorsement);
                        // println!("Endorsement op: {:#?}", injected_endorsement);

                        let preendorsement_quorum_time = quorum_reached(
                            preendorsmenent_statuses,
                            &self.get_endorsing_rights(current_head_level).await?.unwrap(), // TODO
                            constants.consensus_threshold,
                        );
                        // println!(
                        //     "Preendorsement quorum reached: {}",
                        //     preendorsement_quorum_time.is_some()
                        // );

                        let endorsement_summary = EndorsementOperationSummary::new(
                            OffsetDateTime::parse(current_head.timestamp(), &Rfc3339).unwrap(),
                            injected_preendorsement.cloned(),
                            injected_endorsement.cloned(),
                            block_application_stats.clone(),
                            preendorsement_quorum_time,
                        );
                        println!("Summary: {endorsement_summary}");
                        self.endorsmenet_summary_storage
                            .insert(*current_head.level() as i32, endorsement_summary)?;
                        // println!("Inserted summary for: {delegate} at level: {current_head_level}");
                        println!();
                    } else {
                        let endorsement_summary = EndorsementOperationSummary::new(
                            OffsetDateTime::parse(current_head.timestamp(), &Rfc3339).unwrap(),
                            None,
                            None,
                            block_application_stats.clone(),
                            None,
                        );
                        self.endorsmenet_summary_storage
                            .insert(*current_head.level() as i32, endorsement_summary)?;
                    }
                }
            }
            // println!("Endorsement Summaries: {:#?}", self.endorsmenet_summary_storage.inner);
            // println!("Block applications: {:#?}", self.application_statistics_storage.inner);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn get_memopool_endorsement_stats(
        &self,
    ) -> Result<MempoolEndorsementStats, reqwest::Error> {
        reqwest::get(&format!(
            "http://127.0.0.1:{}/dev/shell/automaton/stats/mempool/endorsements",
            self.node.port()
        ))
        .await?
        .json()
        .await
    }

    async fn get_application_stats(
        &self,
        head_level: i32,
    ) -> Result<Vec<BlockApplicationStatistics>, reqwest::Error> {
        reqwest::get(&format!(
            "http://127.0.0.1:{}/dev/shell/automaton/stats/current_head/application?level={head_level}",
            self.node.port()
        ))
        .await?
        .json()
        .await
    }

    async fn get_endorsing_rights(
        &self,
        level: i32,
    ) -> Result<Option<EndorsingRights>, reqwest::Error> {
        reqwest::get(&format!(
            "http://127.0.0.1:{}/chains/main/blocks/head/helpers/endorsing_rights?level={level}",
            self.node.port()
        ))
        .await?
        .json()
        .await
        .map(|res: Vec<EndorsingRights>| res.get(0).cloned())
    }

    async fn get_endorsing_rights_for_delegate(
        &self,
        level: i32,
        delegate: &str,
    ) -> Result<Option<EndorsingRights>, reqwest::Error> {
        reqwest::get(&format!(
            "http://127.0.0.1:{}/chains/main/blocks/head/helpers/endorsing_rights?level={level}&delegate={delegate}",
            self.node.port()
        ))
        .await?
        .json()
        .await
        .map(|res: Vec<EndorsingRights>| res.get(0).cloned())
    }

    async fn get_endorsement_statuses(
        &self,
        level: i32,
        round: i32,
    ) -> Result<BTreeMap<String, EndorsementStatus>, reqwest::Error> {
        reqwest::get(&format!(
            "http://127.0.0.1:{}/dev/shell/automaton/endorsements_status?level={level}&round={round}",
            self.node.port()
        ))
        .await?
        .json()
        .await
    }

    async fn get_preendorsement_statuses(
        &self,
        level: i32,
        round: i32,
    ) -> Result<BTreeMap<String, PreendorsementStatus>, reqwest::Error> {
        reqwest::get(&format!(
            "http://127.0.0.1:{}/dev/shell/automaton/preendorsements_status?level={level}&round={round}",
            self.node.port()
        ))
        .await?
        .json()
        .await
    }

    async fn get_network_constants(&self) -> Result<NetworkConstants, reqwest::Error> {
        reqwest::get(&format!(
            "http://127.0.0.1:{}/chains/main/blocks/head/context/constants",
            self.node.port()
        ))
        .await?
        .json()
        .await
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct NetworkConstants {
    // New fields added in ithaca
    // TODO: ithaca double check the types
    pub consensus_committee_size: u64,
    pub consensus_threshold: u64,
}

impl EndorsementOperationSummary {
    pub fn new(
        current_head_timestamp: OffsetDateTime,
        preendorsement_op_stats: Option<OperationStats>,
        enbdorsement_op_stat: Option<OperationStats>,
        block_stats: Option<BlockApplicationStatistics>,
        preendorsement_quorum_time: Option<i64>,
    ) -> Self {
        let block_received = block_stats.clone().map(|stats| {
            let current_head_nanos = current_head_timestamp.unix_timestamp_nanos();
            ((stats.receive_timestamp as i128) - current_head_nanos) as i64
        });

        let block_application = block_stats.clone().and_then(|stats| {
            stats
                .apply_block_end
                .and_then(|end| stats.apply_block_start.map(|start| (end - start) as i64))
        });

        let preendorsement_injected = preendorsement_op_stats.as_ref().and_then(|op_s| {
            op_s.injected_timestamp.and_then(|inject_time| {
                block_stats.map(|stats| (inject_time as i64) - stats.receive_timestamp)
            })
        });

        let preendorsement_validated = preendorsement_op_stats
            .as_ref()
            .and_then(|op_s| op_s.validation_duration());
    
        let preendorsement_operation_hash_sent = preendorsement_op_stats.as_ref().and_then(|op_s| {
            op_s.first_sent()
                .and_then(|sent| op_s.validation_ended().map(|v_end| sent - v_end))
        });

        let preendorsement_operation_requested = preendorsement_op_stats.as_ref().and_then(|op_s| {
            op_s.first_content_requested_remote()
                .and_then(|op_req| op_s.first_sent().map(|sent| op_req - sent))
        });

        let preendorsement_operation_sent = preendorsement_op_stats.as_ref().and_then(|op_s| {
            op_s.first_content_sent().and_then(|cont_sent| {
                op_s.first_content_requested_remote()
                    .map(|op_req| cont_sent - op_req)
            })
        });

        let preendorsement_operation_hash_received_back = preendorsement_op_stats.as_ref().and_then(|op_s| {
            op_s.second_received().and_then(|received| {
                op_s.first_content_sent()
                    .map(|content_sent| received - content_sent)
            })
        });

        let preendorsement_quorum_reached = preendorsement_quorum_time.and_then(|quorum_time| {
            preendorsement_op_stats.clone().and_then(|stats| {
                stats.injected_timestamp.map(|preendorsement_inject_time| {
                    quorum_time - (preendorsement_inject_time as i64)
                })
            })
        });

        let endorsement_injected = preendorsement_quorum_time.and_then(|quorum_time| {
            enbdorsement_op_stat.clone().and_then(|stats| {
                stats
                    .injected_timestamp
                    .map(|endorsement_inject_time| (endorsement_inject_time as i64) - quorum_time)
            })
        });

        // TODO: debug
        if preendorsement_injected.is_none() || endorsement_injected.is_none() {
            println!(
                "Preendorsement stats: {:#?}",
                preendorsement_op_stats.is_some()
            );
            println!("Endorsement stats: {:#?}", enbdorsement_op_stat.is_some());
        }

        let endorsement_validated = enbdorsement_op_stat
            .as_ref()
            .and_then(|op_s| op_s.validation_duration());

        let endorsement_operation_hash_sent = enbdorsement_op_stat.as_ref().and_then(|op_s| {
            op_s.first_sent()
                .and_then(|sent| op_s.validation_ended().map(|v_end| sent - v_end))
        });

        let endorsement_operation_requested = enbdorsement_op_stat.as_ref().and_then(|op_s| {
            op_s.first_content_requested_remote()
                .and_then(|op_req| op_s.first_sent().map(|sent| op_req - sent))
        });

        let endorsement_operation_sent = enbdorsement_op_stat.as_ref().and_then(|op_s| {
            op_s.first_content_sent().and_then(|cont_sent| {
                op_s.first_content_requested_remote()
                    .map(|op_req| cont_sent - op_req)
            })
        });

        let endorsement_operation_hash_received_back = enbdorsement_op_stat.as_ref().and_then(|op_s| {
            op_s.second_received().and_then(|received| {
                op_s.first_content_sent()
                    .map(|content_sent| received - content_sent)
            })
        });

        Self {
            block_received,
            block_application,
            preendorsement_injected,
            preendorsement_validated,
            preendorsement_operation_hash_sent,
            preendorsement_operation_requested,
            preendorsement_operation_sent,
            preendorsement_operation_hash_received_back,
            preendorsement_quorum_reached,
            endorsement_injected,
            endorsement_validated,
            endorsement_operation_hash_sent,
            endorsement_operation_requested,
            endorsement_operation_sent,
            endorsement_operation_hash_received_back,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EndorsementOperationSummary {
    pub block_application: Option<i64>,
    pub block_received: Option<i64>,
    pub preendorsement_injected: Option<i64>,
    pub preendorsement_validated: Option<i64>,
    pub preendorsement_operation_hash_sent: Option<i64>,
    pub preendorsement_operation_requested: Option<i64>,
    pub preendorsement_operation_sent: Option<i64>,
    pub preendorsement_operation_hash_received_back: Option<i64>,
    pub preendorsement_quorum_reached: Option<i64>,
    pub endorsement_injected: Option<i64>,
    pub endorsement_validated: Option<i64>,
    pub endorsement_operation_hash_sent: Option<i64>,
    pub endorsement_operation_requested: Option<i64>,
    pub endorsement_operation_sent: Option<i64>,
    pub endorsement_operation_hash_received_back: Option<i64>,
}

impl Display for EndorsementOperationSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, r#"
            1. Block Received: {}
            2. Block Application: {}
            3. Preendorsement Injected: {}
            4. Preendorsement Validated: {}
            5. Preendorsement Operation hash sent: {}
            6. Preendorsement Operation hash requested: {}
            7. Preendorsement Operation sent: {}
            8. Preendorsement Operation hash received back: {}
            9. Preendorsement quorum reached: {}
            10. Endorsement Injected:{}
            11. Endorsement Validated: {}
            12. Endorsement Operation hash sent: {}
            13. Endorsement Operation hash requested: {}
            14. Endorsement Operatin sent: {}
            15. Endorsement Operation hash received back: {}"#,
            convert_time_to_unit_string(self.block_received),
            convert_time_to_unit_string(self.block_application),
            convert_time_to_unit_string(self.preendorsement_injected),
            convert_time_to_unit_string(self.preendorsement_validated),
            convert_time_to_unit_string(self.preendorsement_operation_hash_sent),
            convert_time_to_unit_string(self.preendorsement_operation_requested),
            convert_time_to_unit_string(self.preendorsement_operation_sent),
            convert_time_to_unit_string(self.preendorsement_operation_hash_received_back),
            convert_time_to_unit_string(self.preendorsement_quorum_reached),
            convert_time_to_unit_string(self.endorsement_injected),
            convert_time_to_unit_string(self.endorsement_validated),
            convert_time_to_unit_string(self.endorsement_operation_hash_sent),
            convert_time_to_unit_string(self.endorsement_operation_requested),
            convert_time_to_unit_string(self.endorsement_operation_sent),
            convert_time_to_unit_string(self.endorsement_operation_hash_received_back),
        )
    }
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
    Preendorsement,
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

    pub fn second_received(&self) -> Option<i64> {
        self.nodes
            .clone()
            .into_iter()
            .filter_map(|(_, v)| {
                v.received
                    .into_iter()
                    .min_by_key(|v| v.latency)
                    .map(|v| v.latency)
            })
            .sorted_by_key(|val| *val)
            .nth(1)
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

#[derive(Deserialize, Debug, Default, Clone, Serialize, PartialEq)]
pub struct BlockApplicationStatistics {
    pub block_hash: String,
    pub block_timestamp: u64,
    pub receive_timestamp: i64,
    pub baker: Option<String>,
    pub baker_priority: Option<u16>,
    pub download_block_header_start: Option<u64>,
    pub download_block_header_end: Option<u64>,
    pub download_block_operations_start: Option<u64>,
    pub download_block_operations_end: Option<u64>,
    pub load_data_start: Option<u64>,
    pub load_data_end: Option<u64>,
    pub precheck_start: Option<u64>,
    pub precheck_end: Option<u64>,
    pub apply_block_start: Option<u64>,
    pub apply_block_end: Option<u64>,
    pub store_result_start: Option<u64>,
    pub store_result_end: Option<u64>,
    pub send_start: Option<u64>,
    pub send_end: Option<u64>,
    pub protocol_times: Option<BlockApplicationProtocolStatistics>,
    pub injected: Option<u64>,
}

#[derive(Deserialize, Debug, Default, Clone, Serialize, PartialEq)]
pub struct BlockApplicationProtocolStatistics {
    pub apply_start: u64,
    pub operations_decoding_start: u64,
    pub operations_decoding_end: u64,
    // pub operations_application: Vec<Vec<(u64, u64)>>,
    pub operations_metadata_encoding_start: u64,
    pub operations_metadata_encoding_end: u64,
    pub begin_application_start: u64,
    pub begin_application_end: u64,
    pub finalize_block_start: u64,
    pub finalize_block_end: u64,
    pub collect_new_rolls_owner_snapshots_start: u64,
    pub collect_new_rolls_owner_snapshots_end: u64,
    pub commit_start: u64,
    pub commit_end: u64,
    pub apply_end: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct EndorsementStatus {
    // pub block_timestamp: u64,
    pub decoded_time: Option<u64>,
    pub applied_time: Option<u64>,
    pub branch_delayed_time: Option<u64>,
    pub prechecked_time: Option<u64>,
    pub broadcast_time: Option<u64>,
    pub received_contents_time: Option<u64>,
    pub received_hash_time: Option<u64>,
    pub slot: u16,
    pub state: String,
    pub broadcast: bool,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, EnumIter, Serialize, Deserialize)]
pub enum EndorsementState {
    Missing = 0,
    Broadcast = 1,
    Applied = 2,
    Prechecked = 3,
    Decoded = 4,
    Received = 5,
    BranchDelayed = 6,
}

pub fn convert_time_to_unit_string<T>(time: Option<T>) -> String
where
    T: ToPrimitive + PartialOrd + std::ops::Div<Output = T> + std::fmt::Display,
{
    if let Some(time) = time {
        let time = if let Some(time) = time.to_f64() {
            time
        } else {
            return String::from("NaN");
        };

        const MILLISECOND_FACTOR: f64 = 1000.0;
        const MICROSECOND_FACTOR: f64 = 1000000.0;
        const NANOSECOND_FACTOR: f64 = 1000000000.0;

        if time >= NANOSECOND_FACTOR {
            format!("{:.2}s", time / NANOSECOND_FACTOR)
        } else if time >= MICROSECOND_FACTOR {
            format!("{:.2}ms", time / MICROSECOND_FACTOR)
        } else if time >= MILLISECOND_FACTOR {
            format!("{:.2}μs", time / MILLISECOND_FACTOR)
        } else {
            format!("{}ns", time)
        }
    } else {
        String::from("Failed")
    }
}

fn quorum_reached(
    statuses: BTreeMap<String, EndorsementStatus>,
    endorsing_rights: &EndorsingRights,
    threshold: u64,
) -> Option<i64> {
    let endorsing_powers = endorsing_rights.endorsement_powers();
    statuses
        .values()
        .sorted_by_key(|val| val.received_hash_time)
        .filter_map(|status| {
            endorsing_powers.get(&status.slot).and_then(|power| {
                status
                    .received_hash_time
                    .map(|receive_time| (*power, receive_time))
            })
        })
        .reduce(
            |(mut acc, mut receive_time), (power, current_receive_time)| {
                acc += power;
                if u64::from(acc) < threshold {
                    receive_time = current_receive_time;
                }
                (acc, receive_time)
            },
        )
        .map(|(_, receive_time)| receive_time as i64)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_operation_stats_second_received() {
        let stats = OperationStats {
            nodes: vec![
                (
                    String::from("1"),
                    OperationNodeStats {
                        received: vec![OperationNodeCurrentHeadStats {
                            latency: 100,
                            block_level: 1,
                            block_timestamp: 0,
                        }],
                        sent: vec![],
                        content_received: vec![],
                        content_requested: vec![],
                        content_requested_remote: vec![],
                        content_sent: vec![],
                    },
                ),
                (
                    String::from("2"),
                    OperationNodeStats {
                        received: vec![OperationNodeCurrentHeadStats {
                            latency: 150,
                            block_level: 1,
                            block_timestamp: 0,
                        }],
                        sent: vec![],
                        content_received: vec![],
                        content_requested: vec![],
                        content_requested_remote: vec![],
                        content_sent: vec![],
                    },
                ),
                (
                    String::from("3"),
                    OperationNodeStats {
                        received: vec![OperationNodeCurrentHeadStats {
                            latency: 300,
                            block_level: 1,
                            block_timestamp: 0,
                        }],
                        sent: vec![],
                        content_received: vec![],
                        content_requested: vec![],
                        content_requested_remote: vec![],
                        content_sent: vec![],
                    },
                ),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let second_received = stats.second_received();
        assert_eq!(second_received, Some(150));

        let stats = OperationStats {
            nodes: vec![(
                String::from("1"),
                OperationNodeStats {
                    received: vec![OperationNodeCurrentHeadStats {
                        latency: 100,
                        block_level: 1,
                        block_timestamp: 0,
                    }],
                    sent: vec![],
                    content_received: vec![],
                    content_requested: vec![],
                    content_requested_remote: vec![],
                    content_sent: vec![],
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let second_received = stats.second_received();
        assert_eq!(second_received, None);
    }

    #[test]
    fn test_quorum_reached() {}
}
