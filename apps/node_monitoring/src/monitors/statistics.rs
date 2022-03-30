use std::{
    collections::BTreeMap,
    fmt::Display,
    sync::{Arc, RwLock},
};

use itertools::FoldWhile::{Continue, Done};
use itertools::Itertools;
use num::ToPrimitive;
use serde::{Deserialize, Serialize};
use slog::{debug, Logger};
use strum_macros::EnumIter;
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::node::Node;

use super::{delegate::EndorsingRights, resource::ResourceMonitorError};

pub type PayloadHash = String;
pub type OperationStatsMap = BTreeMap<String, OperationStats>;
pub type PreendorsementStatus = EndorsementStatus;

#[derive(Debug, Error)]
pub enum StatisticMonitorError {
    /// Storage error
    #[error("Error while writing into storage, reason: {reason}")]
    StorageError { reason: String },

    #[error("Error in node RPC, reason: {0}")]
    NodeRpcError(#[from] reqwest::Error),

    // TODO: Shouldn't be ResourceMonitorError
    #[error("Other, reason: {0}")]
    OtherError(#[from] ResourceMonitorError),

    #[error("Other error occured, reason: {0}")]
    ParseTimestamp(#[from] time::error::Parse),
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
    pub endorsmenet_summary_storage: LockedBTreeMap<i32, FinalEndorsementSummary>,
    pub application_statistics_storage: LockedBTreeMap<String, BlockApplicationStatistics>,
    pub preendorsement_statistics_storage: LockedBTreeMap<PayloadHash, OperationStats>,
    pub preendorsement_summary_storage: LockedBTreeMap<PayloadHash, PreendorsementOperationSummary>,
    pub preendoesement_quorum_summary_storage:
        LockedBTreeMap<PayloadHash, PreendorsementQuorumSummary>,
    pub delegates: Vec<String>,
    pub last_seen_current_head: Option<String>,
    pub last_seen_round_0_head: Option<String>,
    log: Logger,
}

impl StatisticsMonitor {
    pub fn new(
        node: Node,
        delegates: Vec<String>,
        endorsmenet_summary_storage: LockedBTreeMap<i32, FinalEndorsementSummary>,
        log: Logger,
    ) -> Self {
        Self {
            node,
            endorsmenet_summary_storage,
            application_statistics_storage: LockedBTreeMap::new(),
            preendorsement_statistics_storage: LockedBTreeMap::new(),
            preendorsement_summary_storage: LockedBTreeMap::new(),
            preendoesement_quorum_summary_storage: LockedBTreeMap::new(),
            delegates,
            last_seen_current_head: None,
            last_seen_round_0_head: None,
            log,
        }
    }

    pub async fn parse_statistics(&mut self) -> Result<(), StatisticMonitorError> {
        let current_head = self.node.get_head_data().await?;
        let current_head_level = *current_head.level() as i32;
        let current_head_hash = current_head.block_hash();

        let constants = self.get_network_constants().await?;

        let current_head_timestamp = OffsetDateTime::parse(current_head.timestamp(), &Rfc3339)?;

        // TODO: this should occure only on head change (?)
        let application_statistics = self.get_application_stats(current_head_level).await?;
        for stats in application_statistics {
            self.application_statistics_storage
                .insert(stats.block_hash.clone(), stats)?;
        }

        let block_application_stats = self
            .application_statistics_storage
            .get(current_head.block_hash().to_string())?;

        let is_round_0 = if let Some(block_stats) = block_application_stats.as_ref() {
            block_stats.round == 0
        } else {
            false
        };

        if is_round_0 {
            self.last_seen_round_0_head = Some(current_head_hash.clone());
        }

        // TODO: The blocks stats should be always ready! use another if let and do not provide it in Option further?
        let block_round = block_application_stats.clone().unwrap().round;
        let block_payload_hash = block_application_stats.clone().unwrap().payload_hash;
        debug!(self.log, "Current head - Level: {current_head_level} Round: {block_round} Hash: {current_head_hash}");
        let round_summary = RoundSummary::new(
            current_head_hash,
            current_head_level,
            block_round,
            &block_payload_hash,
        );

        let endorsing_rights_for_level = self.get_endorsing_rights(current_head_level).await?;

        let preendorsmenent_statuses = self
            .get_preendorsement_statuses(current_head_level, block_round)
            .await?;
        let endorsmenent_statuses = self
            .get_endorsement_statuses(current_head_level, block_round)
            .await?;

        let round_0_application_stats =
            if let Some(last_seen_round_0_head) = self.last_seen_round_0_head.as_ref() {
                self.application_statistics_storage
                    .get(last_seen_round_0_head.to_string())?
            } else {
                None
            };

        let preendorsement_quorum_summary = if let Some(preendorsement_quorum_summary) = self
            .preendoesement_quorum_summary_storage
            .get(block_payload_hash.clone())?
        {
            preendorsement_quorum_summary
        } else {
            let preendorsement_quorum_summary = PreendorsementQuorumSummary::new(
                round_summary.clone(),
                preendorsmenent_statuses.clone(),
                endorsing_rights_for_level.clone(),
                round_0_application_stats,
                constants.consensus_threshold,
            );

            // only save when quorum is actually reached!
            if preendorsement_quorum_summary.is_quorum_reached() {
                self.preendoesement_quorum_summary_storage.insert(
                    block_payload_hash.clone(),
                    preendorsement_quorum_summary.clone(),
                )?;
            }
            preendorsement_quorum_summary
        };

        let delegates = self.delegates.clone();

        for delegate in &delegates {
            if let Some(delegate_rigths) = self
                .get_endorsing_rights_for_delegate(current_head_level, delegate)
                .await?
            {
                // TODO: Need a better solution than this indexing
                let delegate_slot = delegate_rigths.delegates[0].get_first_slot();

                if let Some(injected_preendorsement_op_hash) = preendorsmenent_statuses
                    .iter()
                    .filter(|(_, preendorsement)| preendorsement.slot == delegate_slot)
                    .map(|(op_h, _)| op_h)
                    .last()
                {
                    debug!(
                        self.log,
                        "Preendorsement op hash: {injected_preendorsement_op_hash}"
                    );

                    // if there is already a preendorsement summary for the payload hash use that
                    let preendorsement_operation_summary =
                        if let Some(preendorsement_operation_summary) = self
                            .preendorsement_summary_storage
                            .get(block_payload_hash.clone())?
                        {
                            preendorsement_operation_summary
                        } else {
                            // else get preendorsement operation stats from the node
                            let preendorsement_stats = self
                                .get_consensus_operation_stats(injected_preendorsement_op_hash)
                                .await?;
                            if let Some(stats) = preendorsement_stats.clone() {
                                self.preendorsement_statistics_storage
                                    .insert(block_payload_hash.clone(), stats)?;
                            };

                            let preendorsement_operation_summary =
                                PreendorsementOperationSummary::new(
                                    round_summary.clone(),
                                    current_head_timestamp,
                                    preendorsement_stats.clone(),
                                    block_application_stats.clone(),
                                    injected_preendorsement_op_hash.to_string(),
                                );

                            self.preendorsement_summary_storage.insert(
                                block_payload_hash.clone(),
                                preendorsement_operation_summary.clone(),
                            )?;

                            preendorsement_operation_summary
                        };

                    if let Some(injected_endorsement_op_hash) = endorsmenent_statuses
                        .iter()
                        .filter(|(_, endorsement)| endorsement.slot == delegate_slot)
                        .map(|(op_h, _)| op_h)
                        .last()
                    {
                        debug!(
                            self.log,
                            "Endorsement op hash: {injected_endorsement_op_hash}"
                        );

                        let injected_endorsement = self
                            .get_consensus_operation_stats(injected_endorsement_op_hash)
                            .await?;

                        let endorsement_operation_summary = EndorsementOperationSummary::new(
                            round_summary.clone(),
                            current_head_timestamp,
                            injected_endorsement,
                            block_application_stats.clone(),
                            preendorsement_quorum_summary
                                .clone()
                                .preendorsement_quorum_timestamp,
                            injected_endorsement_op_hash.to_string(),
                        );

                        self.create_or_modify_final_summary(
                            &round_summary,
                            Some(preendorsement_operation_summary),
                            preendorsement_quorum_summary.clone(),
                            Some(endorsement_operation_summary),
                        )?;
                    } else {
                        self.create_or_modify_final_summary(
                            &round_summary,
                            Some(preendorsement_operation_summary),
                            preendorsement_quorum_summary.clone(),
                            None,
                        )?;
                    }
                } else {
                    self.create_or_modify_final_summary(
                        &round_summary,
                        None,
                        preendorsement_quorum_summary.clone(),
                        None,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn create_or_modify_final_summary(
        &mut self,
        round_summary: &RoundSummary,
        preendorsement_operation_summary: Option<PreendorsementOperationSummary>,
        preendorsement_quorum_summary: PreendorsementQuorumSummary,
        endorsement_operation_summary: Option<EndorsementOperationSummary>,
    ) -> Result<(), StatisticMonitorError> {
        let RoundSummary {
            block_round,
            block_level,
            ..
        } = round_summary;

        let final_endorsement_summary = match self.endorsmenet_summary_storage.get(*block_level) {
            Ok(Some(mut final_endorsement_summary)) => {
                // update the values
                final_endorsement_summary
                    .preendorsement_operation_summary
                    .insert(*block_round, preendorsement_operation_summary);
                final_endorsement_summary
                    .preendorsement_quorum_summary
                    .insert(*block_round, preendorsement_quorum_summary);
                final_endorsement_summary
                    .endorsement_operations_summary
                    .insert(*block_round, endorsement_operation_summary);
                final_endorsement_summary.last_seen_round = *block_round;
                final_endorsement_summary
            }
            Ok(None) => {
                let mut endorsement_operations_summary_map = BTreeMap::new();
                let mut preendorsement_quorum_summary_map = BTreeMap::new();
                let mut preendorsement_operations_summart_map = BTreeMap::new();
                preendorsement_operations_summart_map
                    .insert(*block_round, preendorsement_operation_summary);
                preendorsement_quorum_summary_map
                    .insert(*block_round, preendorsement_quorum_summary);
                endorsement_operations_summary_map
                    .insert(*block_round, endorsement_operation_summary);
                FinalEndorsementSummary::new(
                    preendorsement_operations_summart_map,
                    preendorsement_quorum_summary_map,
                    endorsement_operations_summary_map,
                    *block_round,
                )
            }
            Err(e) => return Err(e),
        };
        debug!(self.log, "Summary:\n {final_endorsement_summary}");
        self.endorsmenet_summary_storage
            .insert(*block_level, final_endorsement_summary)?;
        Ok(())
    }

    async fn get_consensus_operation_stats(
        &self,
        op_hash: &str,
    ) -> Result<Option<OperationStats>, reqwest::Error> {
        reqwest::get(&format!(
            "http://127.0.0.1:{}/dev/shell/automaton/mempool/operation_stats?hash={op_hash}",
            self.node.port()
        ))
        .await?
        .json()
        .await
        .map(|res: OperationStatsMap| res.values().last().cloned())
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
        round_summary: RoundSummary,
        current_head_timestamp: OffsetDateTime,
        endorsement_op_stat: Option<OperationStats>,
        block_stats: Option<BlockApplicationStatistics>,
        preendorsement_quorum_time: Option<i64>,
        operation_hash: String,
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

        let endorsement_injected = preendorsement_quorum_time.and_then(|quorum_time| {
            endorsement_op_stat.clone().and_then(|stats| {
                stats
                    .injected_timestamp
                    .and_then(|endorsement_inject_time| {
                        block_stats.map(|block_stats| {
                            if (block_stats.block_timestamp as i64) > quorum_time {
                                (endorsement_inject_time - block_stats.block_timestamp) as i64
                            } else {
                                (endorsement_inject_time as i64) - quorum_time
                            }
                        })
                    })
            })
        });

        let endorsement_validated = endorsement_op_stat
            .as_ref()
            .and_then(|op_s| op_s.validation_duration());

        let endorsement_operation_hash_sent = endorsement_op_stat.as_ref().and_then(|op_s| {
            op_s.first_sent()
                .and_then(|sent| op_s.validation_ended().map(|v_end| sent - v_end))
        });

        let endorsement_operation_requested = endorsement_op_stat.as_ref().and_then(|op_s| {
            op_s.first_content_requested_remote()
                .and_then(|op_req| op_s.first_sent().map(|sent| op_req - sent))
        });

        let endorsement_operation_sent = endorsement_op_stat.as_ref().and_then(|op_s| {
            op_s.first_content_sent().and_then(|cont_sent| {
                op_s.first_content_requested_remote()
                    .map(|op_req| cont_sent - op_req)
            })
        });

        let endorsement_operation_hash_received_back =
            endorsement_op_stat.as_ref().and_then(|op_s| {
                op_s.second_received().and_then(|received| {
                    op_s.first_content_sent()
                        .map(|content_sent| received - content_sent)
                })
            });

        Self {
            round_summary,
            block_received,
            block_application,
            endorsement_injected,
            endorsement_validated,
            endorsement_operation_hash_sent,
            endorsement_operation_requested,
            endorsement_operation_sent,
            endorsement_operation_hash_received_back,
            operation_hash,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RoundSummary {
    pub block_hash: String,
    pub block_level: i32,
    pub block_round: i32,
    pub block_payload_hash: String,
}

impl RoundSummary {
    pub fn new(
        block_hash: &str,
        block_level: i32,
        block_round: i32,
        block_payload_hash: &str,
    ) -> Self {
        Self {
            block_hash: block_hash.to_string(),
            block_level,
            block_round,
            block_payload_hash: block_payload_hash.to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PreendorsementQuorumSummary {
    pub round_summary: RoundSummary,
    pub preendorsement_quorum_timestamp: Option<i64>,
    pub preendorsement_quorum_reached: Option<i64>,
}

impl Display for PreendorsementQuorumSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_quorum_reached() {
            writeln!(
                f,
                "Quorum REACHED in {}",
                convert_time_to_unit_string(self.preendorsement_quorum_reached),
            )
        } else {
            writeln!(f, "Quorum NOT REACHED",)
        }
    }
}

impl PreendorsementQuorumSummary {
    fn new(
        round_summary: RoundSummary,
        statuses: BTreeMap<String, EndorsementStatus>,
        endorsing_rights: Option<EndorsingRights>,
        block_application_stats: Option<BlockApplicationStatistics>,
        threshold: u64,
    ) -> Self {
        // Uses branch_delayed time or applied_time
        let preendorsement_quorum = endorsing_rights.map(|rights| {
            let endorsing_powers = rights.endorsement_powers();
            statuses
                .values()
                .filter_map(|status| {
                    endorsing_powers.get(&status.slot).and_then(|power| {
                        status
                            .branch_delayed_time
                            .map(|branch_delayed_time| (*power, branch_delayed_time))
                            .or_else(|| {
                                status
                                    .applied_time
                                    .map(|applied_time| (*power, applied_time))
                            })
                    })
                })
                .sorted_by_key(|val| val.1)
                .fold_while((0, 0), |(acc, time), (power, current_time)| {
                    if acc < threshold {
                        Continue((acc + u64::from(power), current_time))
                    } else {
                        Done((acc, time))
                    }
                })
                .into_inner()
        });

        let (endorsing_power, preendorsement_quorum_timestamp) =
            if let Some((endorsing_power, quorum_timestamp)) = preendorsement_quorum {
                if endorsing_power > threshold {
                    (Some(endorsing_power), Some(quorum_timestamp as i64))
                } else {
                    (Some(endorsing_power), None)
                }
            } else {
                (None, None)
            };

        println!(
            "Endorsing power at the end of quorum calculation: {:#?}",
            endorsing_power
        );

        let preendorsement_quorum_reached =
            preendorsement_quorum_timestamp.and_then(|quorum_time| {
                block_application_stats
                    .clone()
                    .map(|stats| quorum_time - (stats.block_timestamp as i64))
            });

        Self {
            round_summary,
            preendorsement_quorum_timestamp,
            preendorsement_quorum_reached,
        }
    }

    pub fn is_quorum_reached(&self) -> bool {
        self.preendorsement_quorum_reached.is_some()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PreendorsementOperationSummary {
    pub round_summary: RoundSummary,
    pub block_application: Option<i64>,
    pub block_received: Option<i64>,
    pub preendorsement_injected: Option<i64>,
    pub preendorsement_validated: Option<i64>,
    pub preendorsement_operation_hash_sent: Option<i64>,
    pub preendorsement_operation_requested: Option<i64>,
    pub preendorsement_operation_sent: Option<i64>,
    pub preendorsement_operation_hash_received_back: Option<i64>,
    pub operation_hash: String,
}

impl PreendorsementOperationSummary {
    pub fn new(
        round_summary: RoundSummary,
        current_head_timestamp: OffsetDateTime,
        preendorsement_op_stats: Option<OperationStats>,
        block_stats: Option<BlockApplicationStatistics>,
        operation_hash: String,
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

        let preendorsement_operation_hash_sent =
            preendorsement_op_stats.as_ref().and_then(|op_s| {
                op_s.first_sent()
                    .and_then(|sent| op_s.validation_ended().map(|v_end| sent - v_end))
            });

        let preendorsement_operation_requested =
            preendorsement_op_stats.as_ref().and_then(|op_s| {
                op_s.first_content_requested_remote()
                    .and_then(|op_req| op_s.first_sent().map(|sent| op_req - sent))
            });

        let preendorsement_operation_sent = preendorsement_op_stats.as_ref().and_then(|op_s| {
            op_s.first_content_sent().and_then(|cont_sent| {
                op_s.first_content_requested_remote()
                    .map(|op_req| cont_sent - op_req)
            })
        });

        let preendorsement_operation_hash_received_back =
            preendorsement_op_stats.as_ref().and_then(|op_s| {
                op_s.second_received().and_then(|received| {
                    op_s.first_content_sent()
                        .map(|content_sent| received - content_sent)
                })
            });

        Self {
            round_summary,
            block_received,
            block_application,
            preendorsement_injected,
            preendorsement_validated,
            preendorsement_operation_hash_sent,
            preendorsement_operation_requested,
            preendorsement_operation_sent,
            preendorsement_operation_hash_received_back,
            operation_hash,
        }
    }
}

impl Display for PreendorsementOperationSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            r#"
            Operation hash: {}
            Block hash: {}
            Level: {}
            Round: {}
            Payload hash: {}"#,
            self.operation_hash,
            self.round_summary.block_hash,
            self.round_summary.block_level,
            self.round_summary.block_round,
            self.round_summary.block_payload_hash,
        )?;
        writeln!(f)?;
        writeln!(
            f,
            r#"
            1. Block Received: {}
            2. Block Application: {}
            3. Preendorsement Injected via RPC: {}
            4. Preendorsement Validated: {}
            5. Preendorsement Operation hash sent: {}
            6. Preendorsement Operation hash requested: {}
            7. Preendorsement Operation sent: {}
            8. Preendorsement Operation hash received back: {}"#,
            convert_time_to_unit_string(self.block_received),
            convert_time_to_unit_string(self.block_application),
            convert_time_to_unit_string(self.preendorsement_injected),
            convert_time_to_unit_string(self.preendorsement_validated),
            convert_time_to_unit_string(self.preendorsement_operation_hash_sent),
            convert_time_to_unit_string(self.preendorsement_operation_requested),
            convert_time_to_unit_string(self.preendorsement_operation_sent),
            convert_time_to_unit_string(self.preendorsement_operation_hash_received_back),
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EndorsementOperationSummary {
    pub round_summary: RoundSummary,
    pub operation_hash: String,
    pub block_application: Option<i64>,
    pub block_received: Option<i64>,
    pub endorsement_injected: Option<i64>,
    pub endorsement_validated: Option<i64>,
    pub endorsement_operation_hash_sent: Option<i64>,
    pub endorsement_operation_requested: Option<i64>,
    pub endorsement_operation_sent: Option<i64>,
    pub endorsement_operation_hash_received_back: Option<i64>,
}

impl Display for EndorsementOperationSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            r#"
            Operation hash: {}
            Block hash: {}
            Level: {}
            Round: {} 
            Payload hash: {}"#,
            self.operation_hash,
            self.round_summary.block_hash,
            self.round_summary.block_level,
            self.round_summary.block_round,
            self.round_summary.block_payload_hash,
        )?;
        writeln!(f)?;
        writeln!(
            f,
            r#"
            1. Block Received: {}
            2. Block Application: {}
            3. Endorsement Injected via RPC: {}
            4. Endorsement Validated: {}
            5. Endorsement Operation hash sent: {}
            6. Endorsement Operation hash requested: {}
            7. Endorsement Operatin sent: {}
            8. Endorsement Operation hash received back: {}"#,
            convert_time_to_unit_string(self.block_received),
            convert_time_to_unit_string(self.block_application),
            convert_time_to_unit_string(self.endorsement_injected),
            convert_time_to_unit_string(self.endorsement_validated),
            convert_time_to_unit_string(self.endorsement_operation_hash_sent),
            convert_time_to_unit_string(self.endorsement_operation_requested),
            convert_time_to_unit_string(self.endorsement_operation_sent),
            convert_time_to_unit_string(self.endorsement_operation_hash_received_back),
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct FinalEndorsementSummary {
    pub preendorsement_operation_summary: BTreeMap<i32, Option<PreendorsementOperationSummary>>,
    pub preendorsement_quorum_summary: BTreeMap<i32, PreendorsementQuorumSummary>,
    pub endorsement_operations_summary: BTreeMap<i32, Option<EndorsementOperationSummary>>,
    pub last_seen_round: i32,
}

impl Display for FinalEndorsementSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for round in 0..=self.last_seen_round {
            writeln!(f, "~~~~~~~~~")?;
            // All the 3 maps are updated on a new round, so if there was no proposal for round r, there should not
            // be a key in any of the maps so checking one is enough
            if !self.preendorsement_quorum_summary.contains_key(&round) {
                writeln!(f, "Round {round}: Proposal missing")?;
            } else {
                writeln!(f, "Round {round}:")?;
                // Preendorsement operation for the round
                if let Some(summary) = self.preendorsement_operation_summary.get(&round) {
                    match summary {
                        Some(preendorsement_operation_summary) => {
                            let PreendorsementOperationSummary { round_summary, .. } =
                                preendorsement_operation_summary;

                            let RoundSummary { block_round, .. } = round_summary;

                            if block_round == &round {
                                writeln!(
                                    f,
                                    "Preendorsement operation: {preendorsement_operation_summary}"
                                )?
                            } else {
                                writeln!(
                                    f,
                                    "Preendorsement operation: Locked on round {block_round}"
                                )?
                            }
                        }
                        None => writeln!(f, "Preendorsement operation: Not found")?,
                    }
                }
                // Preendorsement quorum for the round
                if let Some(summary) = self.preendorsement_quorum_summary.get(&round) {
                    let PreendorsementQuorumSummary { round_summary, .. } = summary;

                    let RoundSummary {
                        block_round,
                        block_payload_hash,
                        ..
                    } = round_summary;

                    if block_round == &round {
                        writeln!(f, "Preendorsement quorum (round {round} - {block_payload_hash}): {summary}")?
                    } else {
                        writeln!(f, "Preendorsement quorum (round {round} - {block_payload_hash}): Locked on round {block_round}")?
                    }
                }
                // Endorsement operation for the round
                if let Some(summary) = self.endorsement_operations_summary.get(&round) {
                    match summary {
                        Some(endorsement_operation_summary) => {
                            writeln!(f, "Endorsement operation: {endorsement_operation_summary}")?
                        }
                        None => writeln!(f, "Endorsement operation: Not found")?,
                    }
                }
            }
            writeln!(f, "~~~~~~~~~")?;
        }

        Ok(())
    }
}

impl FinalEndorsementSummary {
    pub fn new(
        preendorsement_operation_summary: BTreeMap<i32, Option<PreendorsementOperationSummary>>,
        preendorsement_quorum_summary: BTreeMap<i32, PreendorsementQuorumSummary>,
        endorsement_operations_summary: BTreeMap<i32, Option<EndorsementOperationSummary>>,
        last_seen_round: i32,
    ) -> Self {
        Self {
            preendorsement_operation_summary,
            preendorsement_quorum_summary,
            endorsement_operations_summary,
            last_seen_round,
        }
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
    Outdated,
}

impl OperationStats {
    pub fn _node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn _is_injected(&self) -> bool {
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
    pub round: i32,
    pub payload_hash: String,
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
    pub broadcast: BroadcastState,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum BroadcastState {
    Pending,
    Broadcast,
    NotNeeded,
}

impl Default for BroadcastState {
    fn default() -> Self {
        Self::Pending
    }
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

#[cfg(test)]
mod test {
    use crate::monitors::delegate::DelegateEndorsingRights;

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
    fn test_quorum_reached() {
        let round_summary = RoundSummary::new("Block1", 1, 0, "PayloadHash1");

        let endorsing_rights = EndorsingRights {
            level: 1,
            delegates: vec![
                DelegateEndorsingRights {
                    delegate: String::from("baker1"),
                    first_slot: 0,
                    endorsing_power: 25,
                },
                DelegateEndorsingRights {
                    delegate: String::from("baker2"),
                    first_slot: 50,
                    endorsing_power: 50,
                },
                DelegateEndorsingRights {
                    delegate: String::from("baker3"),
                    first_slot: 25,
                    endorsing_power: 25,
                },
                DelegateEndorsingRights {
                    delegate: String::from("baker4"),
                    first_slot: 60,
                    endorsing_power: 40,
                },
            ],
        };

        let block_application_stats = BlockApplicationStatistics {
            block_timestamp: 1000,
            ..Default::default()
        };

        // Case where quorum should not be reachable just yet
        let mut statuses: BTreeMap<String, EndorsementStatus> = vec![(
            String::from("Ophash1"),
            EndorsementStatus {
                slot: 0,
                applied_time: Some(1020),
                state: String::from("applied"),
                ..Default::default()
            },
        )]
        .into_iter()
        .collect();

        let quorum_summary = PreendorsementQuorumSummary::new(
            round_summary.clone(),
            statuses.clone(),
            Some(endorsing_rights.clone()),
            Some(block_application_stats.clone()),
            50,
        );

        assert_eq!(quorum_summary.preendorsement_quorum_reached, None);
        assert_eq!(quorum_summary.preendorsement_quorum_timestamp, None);

        // insert a preendoesement with enough power to reach quorum
        statuses.insert(
            String::from("Ophash2"),
            EndorsementStatus {
                slot: 60,
                applied_time: Some(1100),
                state: String::from("applied"),
                ..Default::default()
            },
        );

        let quorum_summary = PreendorsementQuorumSummary::new(
            round_summary.clone(),
            statuses.clone(),
            Some(endorsing_rights.clone()),
            Some(block_application_stats.clone()),
            50,
        );

        assert_eq!(quorum_summary.preendorsement_quorum_reached, Some(100));
        assert_eq!(quorum_summary.preendorsement_quorum_timestamp, Some(1100));

        // insert another preendorsement, the quorum is already reached, so the result should stay the same
        statuses.insert(
            String::from("Ophash3"),
            EndorsementStatus {
                slot: 25,
                applied_time: Some(1200),
                state: String::from("applied"),
                ..Default::default()
            },
        );

        assert_eq!(quorum_summary.preendorsement_quorum_reached, Some(100));
        assert_eq!(quorum_summary.preendorsement_quorum_timestamp, Some(1100));

        // insert another preendorsement, this time one that was received earlier as branch_delayed
        statuses.insert(
            String::from("Ophash4"),
            EndorsementStatus {
                slot: 50,
                branch_delayed_time: Some(1050),
                state: String::from("applied"),
                ..Default::default()
            },
        );

        let quorum_summary = PreendorsementQuorumSummary::new(
            round_summary,
            statuses.clone(),
            Some(endorsing_rights),
            Some(block_application_stats),
            50,
        );

        assert_eq!(quorum_summary.preendorsement_quorum_reached, Some(50));
        assert_eq!(quorum_summary.preendorsement_quorum_timestamp, Some(1050));
    }
}
