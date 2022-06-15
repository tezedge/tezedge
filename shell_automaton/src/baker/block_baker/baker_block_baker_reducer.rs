// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;

use crypto::hash::{
    BlockPayloadHash, HashTrait, HashType, OperationListHash, OperationMetadataListListHash,
};
use tezos_encoding::types::SizedBytes;
use tezos_messages::p2p::encoding::block_header::BlockHeaderBuilder;

use crate::baker::{BakerState, ElectedBlock};
use crate::block_applier::BlockApplierApplyState;
use crate::mempool::{MempoolState, OperationKind};
use crate::{Action, ActionWithMeta, State};

use super::{BakerBlockBakerState, BakingSlot, BuiltBlock};

fn set_elected_block_operations(baker: &mut BakerState, mempool: &MempoolState) {
    baker.elected_block = baker.elected_block.take().and_then(|mut block| {
        if !block.operations.is_empty() {
            return Some(block);
        }
        let applied = &mempool.validated_operations.applied;
        let applied = applied
            .iter()
            .map(|v| v.hash.clone())
            .collect::<BTreeSet<_>>();

        let ops = &mempool.validated_operations.ops;

        let empty_operations = vec![vec![], vec![], vec![], vec![]];
        let operations = applied
            .into_iter()
            .filter_map(|hash| Some((ops.get(&hash)?, hash)))
            .filter(|(op, hash)| {
                if !OperationKind::from_operation_content_raw(op.data().as_ref())
                    .is_consensus_operation()
                {
                    return true;
                }
                let op_state = mempool.operations_state.get(hash);
                op_state
                    .and_then(|op| {
                        let op = op.operation_decoded_contents.as_ref()?;

                        let (level, round) = op.level_round()?;
                        Some(
                            level == block.header().level()
                                && round == block.round()
                                && op.payload()? == block.payload_hash(),
                        )
                    })
                    .unwrap_or(false)
            })
            .fold(empty_operations, |mut r, (op, hash)| {
                let container = match OperationKind::from_operation_content_raw(op.data().as_ref())
                {
                    OperationKind::Unknown
                    | OperationKind::Preendorsement
                    | OperationKind::FailingNoop
                    | OperationKind::EndorsementWithSlot => return r,
                    OperationKind::Endorsement => &mut r[0],
                    OperationKind::Proposals | OperationKind::Ballot => &mut r[1],
                    OperationKind::SeedNonceRevelation
                    | OperationKind::DoublePreendorsementEvidence
                    | OperationKind::DoubleEndorsementEvidence
                    | OperationKind::DoubleBakingEvidence
                    | OperationKind::ActivateAccount => {
                        // TODO(zura): do we need it???
                        // if op.signature.is_none() {
                        //     op.signature = Some(Signature(vec![0; 64]));
                        // }
                        &mut r[2]
                    }
                    OperationKind::Reveal
                    | OperationKind::Transaction
                    | OperationKind::Origination
                    | OperationKind::Delegation
                    | OperationKind::RegisterGlobalConstant
                    | OperationKind::SetDepositsLimit => &mut r[3],
                };
                container.push((op, hash));
                r
            });

        block.operations = operations
            .iter()
            .map(|ops| ops.into_iter().map(|(op, _)| (*op).clone()).collect())
            .collect();
        block.non_consensus_op_hashes = operations
            .into_iter()
            .skip(1)
            .flatten()
            .map(|(_, hash)| hash)
            .collect();

        Some(block)
    });
}

pub fn baker_block_baker_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::BlockApplierApplySuccess(_) => {
            let new_block = match &state.block_applier.current {
                BlockApplierApplyState::Success { block, .. } => &*block,
                _ => return,
            };
            let mempool = &state.mempool;
            for (_, baker) in state.bakers.iter_mut() {
                if let Some(elected_block) = baker.elected_block.as_ref() {
                    if elected_block.header().level() < new_block.header.level() {
                        baker.elected_block = None;
                        continue;
                    }
                    set_elected_block_operations(baker, mempool);
                }
            }
        }
        Action::CurrentHeadRehydrated(_) | Action::CurrentHeadUpdate(_) => {
            for (_, baker) in state.bakers.iter_mut() {
                baker.block_baker = BakerBlockBakerState::Idle {
                    time: action.time_as_nanos(),
                };
                let head = state.current_head.get();
                let pred = state.current_head.get_pred();
                baker.elected_block = baker
                    .elected_block
                    .take()
                    .and_then(|block| {
                        if block.header().level() < head?.header.level() {
                            None
                        } else {
                            Some(block)
                        }
                    })
                    .or_else(|| {
                        if pred?.header.proto() != head?.header.proto() {
                            let block = head?.clone();
                            let payload_hash =
                                block.header.payload_hash().clone().or_else(|| {
                                    BlockPayloadHash::try_from_bytes(
                                        &[0; HashType::BlockPayloadHash.size()],
                                    )
                                    .ok()
                                })?;
                            let ops_metadata_hash = state
                                .current_head
                                .ops_metadata_hash()
                                .cloned()
                                .or_else(|| {
                                    OperationMetadataListListHash::try_from_bytes(
                                        &[0; HashType::OperationMetadataListListHash.size()],
                                    )
                                    .ok()
                                })?;
                            Some(ElectedBlock {
                                block,
                                round: 0,
                                payload_hash,
                                block_metadata_hash: state
                                    .current_head
                                    .block_metadata_hash()?
                                    .clone(),
                                ops_metadata_hash,
                                operations: vec![],
                                non_consensus_op_hashes: vec![],
                            })
                        } else {
                            None
                        }
                    });
            }
        }
        Action::BakerBlockBakerRightsGetPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                baker.block_baker = BakerBlockBakerState::RightsGetPending {
                    time: action.time_as_nanos(),
                    slots: None,
                    next_slots: None,
                };
            }
        }
        Action::BakerBlockBakerRightsGetCurrentLevelSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &mut baker.block_baker {
                    BakerBlockBakerState::RightsGetPending { slots, .. } => {
                        *slots = Some(content.slots.clone());
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerRightsGetNextLevelSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &mut baker.block_baker {
                    BakerBlockBakerState::RightsGetPending { next_slots, .. } => {
                        *next_slots = Some(content.slots.clone());
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerRightsGetSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &mut baker.block_baker {
                    BakerBlockBakerState::RightsGetPending {
                        slots, next_slots, ..
                    } => match (slots, next_slots) {
                        (Some(slots), Some(next_slots)) => {
                            baker.block_baker = BakerBlockBakerState::RightsGetSuccess {
                                time: action.time_as_nanos(),
                                slots: std::mem::take(slots),
                                next_slots: std::mem::take(next_slots),
                            };
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerRightsNoRights(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let time = action.time_as_nanos();
                baker.block_baker = BakerBlockBakerState::NoRights { time };
            }
        }
        Action::BakerBlockBakerTimeoutPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::RightsGetSuccess {
                        slots, next_slots, ..
                    } => {
                        let constants = match state.current_head.constants() {
                            Some(v) => v,
                            None => return,
                        };
                        let current_slot = match state.current_head.round() {
                            Some(round) => {
                                (round as u32 % constants.consensus_committee_size) as u16
                            }
                            None => 0,
                        };
                        let next_round = slots
                            .into_iter()
                            .map(|slot| *slot)
                            .find(|slot| *slot > current_slot)
                            .and_then(|slot| {
                                let pred = state.current_head.get()?;
                                let round = pred.header.fitness().round()?;
                                let timestamp = pred.header.timestamp().as_u64();
                                let timestamp = timestamp * 1_000_000_000;

                                let rounds_left = slot.checked_sub(current_slot)? as i32;
                                let target_round = round + rounds_left;
                                let time_left = calc_time_until_round(
                                    round as u64,
                                    target_round as u64,
                                    constants.min_block_delay,
                                    constants.delay_increment_per_round,
                                );
                                let timeout = timestamp + time_left;
                                Some(BakingSlot {
                                    round: target_round as u32,
                                    timeout,
                                })
                            });

                        let next_level = next_slots.get(0).cloned().and_then(|slot| {
                            let pred = baker
                                .elected_block_header_with_hash()
                                .or_else(|| state.current_head.get())?;
                            let timestamp = pred.header.timestamp().as_u64();
                            let timestamp = timestamp * 1_000_000_000;

                            let time_left = constants.min_block_delay * 1_000_000_000
                                + calc_time_until_round(
                                    0,
                                    slot as u64,
                                    constants.min_block_delay,
                                    constants.delay_increment_per_round,
                                );
                            let timeout = timestamp + time_left;
                            Some(BakingSlot {
                                round: slot as u32,
                                timeout,
                            })
                        });

                        baker.block_baker = BakerBlockBakerState::TimeoutPending {
                            time: action.time_as_nanos(),
                            next_round,
                            next_level,
                            next_level_timeout_notified: false,
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerNextLevelTimeoutSuccessQuorumPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &mut baker.block_baker {
                    BakerBlockBakerState::TimeoutPending {
                        next_level_timeout_notified,
                        ..
                    } => {
                        *next_level_timeout_notified = true;
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerBakeNextRound(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::TimeoutPending { next_round, .. } => {
                        let next_round = match next_round {
                            Some(v) => v,
                            None => return,
                        };
                        let timestamp = next_round.timeout / 1_000_000_000;
                        baker.block_baker = BakerBlockBakerState::BakeNextRound {
                            time: action.time_as_nanos(),
                            round: next_round.round,
                            block_timestamp: (timestamp as i64).into(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::MempoolQuorumReached(_) => {
            let head = state.current_head.get();
            let payload_hash = state.current_head.payload_hash();
            let block_metadata_hash = state.current_head.block_metadata_hash();
            let ops_metadata_hash = state.current_head.ops_metadata_hash();

            state
                .bakers
                .iter_mut()
                .filter(|(_, baker_state)| baker_state.elected_block.is_none())
                .try_for_each(|(_, baker_state)| {
                    let block = head?.clone();
                    let round = block.header.fitness().round()?;

                    baker_state.elected_block = Some(ElectedBlock {
                        block,
                        round,
                        payload_hash: payload_hash?.clone(),
                        block_metadata_hash: block_metadata_hash?.clone(),
                        ops_metadata_hash: ops_metadata_hash?.clone(),
                        operations: vec![],
                        non_consensus_op_hashes: vec![],
                    });
                    Some(())
                });
        }
        Action::BakerBlockBakerBakeNextLevel(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::TimeoutPending { next_level, .. } => {
                        let next_level = match next_level {
                            Some(v) => v,
                            None => return,
                        };
                        let round = next_level.round;
                        let timestamp = next_level.timeout / 1_000_000_000;

                        if baker
                            .elected_block
                            .as_ref()
                            .map_or(false, |v| v.operations.is_empty())
                        {
                            set_elected_block_operations(baker, &state.mempool);
                        }
                        baker.block_baker = BakerBlockBakerState::BakeNextLevel {
                            time: action.time_as_nanos(),
                            round,
                            block_timestamp: (timestamp as i64).into(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerBuildBlockSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let head = match state.current_head.get() {
                    Some(v) => v,
                    None => return,
                };
                let constants = match state.current_head.constants() {
                    Some(v) => v,
                    None => return,
                };
                let block = match &baker.block_baker {
                    BakerBlockBakerState::BakeNextLevel {
                        round,
                        block_timestamp,
                        ..
                    } => {
                        let payload_round = *round;
                        let elected_block = match &baker.elected_block {
                            Some(v) => v,
                            None => return,
                        };
                        let payload_hash = BlockPayloadHash::calculate(
                            elected_block.hash(),
                            payload_round,
                            &OperationListHash::calculate(&elected_block.non_consensus_op_hashes)
                                .unwrap(),
                        )
                        .unwrap();
                        let predecessor_max_operations_ttl =
                            state.current_head.max_operations_ttl().unwrap_or(1);
                        BuiltBlock {
                            round: payload_round as i32,
                            payload_round: payload_round as i32,
                            timestamp: *block_timestamp,
                            payload_hash,
                            proof_of_work_nonce: SizedBytes([0; 8]),
                            seed_nonce_hash: content.seed_nonce_hash.clone(),
                            liquidity_baking_escape_vote: baker.liquidity_baking_escape_vote,
                            operations: elected_block.operations.clone(),
                            predecessor_header: elected_block.header().clone(),
                            predecessor_max_operations_ttl,
                            pred_block_metadata_hash: elected_block.block_metadata_hash.clone(),
                            pred_ops_metadata_hash: elected_block.ops_metadata_hash.clone(),
                        }
                    }
                    BakerBlockBakerState::BakeNextRound {
                        round,
                        block_timestamp,
                        ..
                    } => {
                        let predecessor_max_operations_ttl = state
                            .current_head
                            .predecessor_max_operations_ttl()
                            .unwrap_or(1);
                        let built_block = baker
                            .locked_payload
                            .as_ref()
                            .filter(|p| p.level() == head.header.level())
                            .map(|p| BuiltBlock {
                                round: *round as i32,
                                payload_round: p.payload_round,
                                timestamp: *block_timestamp,
                                payload_hash: p.payload_hash.clone(),
                                proof_of_work_nonce: SizedBytes([0; 8]),
                                seed_nonce_hash: content.seed_nonce_hash.clone(),
                                liquidity_baking_escape_vote: baker.liquidity_baking_escape_vote,
                                operations: p.operations.clone(),
                                predecessor_header: p.pred_header.clone(),
                                predecessor_max_operations_ttl,
                                pred_block_metadata_hash: p.pred_block_metadata_hash.clone(),
                                pred_ops_metadata_hash: p.pred_ops_metadata_hash.clone(),
                            })
                            .or_else(|| {
                                let current_head = &state.current_head;
                                Some(BuiltBlock {
                                    round: *round as i32,
                                    payload_round: current_head.payload_round()?,
                                    timestamp: *block_timestamp,
                                    payload_hash: current_head.payload_hash()?.clone(),
                                    proof_of_work_nonce: SizedBytes([0; 8]),
                                    seed_nonce_hash: content.seed_nonce_hash.clone(),
                                    liquidity_baking_escape_vote: baker
                                        .liquidity_baking_escape_vote,
                                    operations: current_head.operations()?.clone(),
                                    predecessor_header: (*current_head.get_pred()?.header).clone(),
                                    predecessor_max_operations_ttl,
                                    pred_block_metadata_hash: current_head
                                        .pred_block_metadata_hash()?
                                        .clone(),
                                    pred_ops_metadata_hash: current_head
                                        .pred_ops_metadata_hash()?
                                        .clone(),
                                })
                            });
                        match built_block {
                            Some(v) => v,
                            None => return,
                        }
                    }
                    _ => return,
                };
                baker.block_baker = BakerBlockBakerState::BuildBlock {
                    time: action.time_as_nanos(),
                    block,
                };
            }
        }
        Action::BakerBlockBakerPreapplyPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::BuildBlock { .. } => {
                        baker.block_baker = BakerBlockBakerState::PreapplyPending {
                            time: action.time_as_nanos(),
                            protocol_req_id: content.protocol_req_id,
                            request: content.request.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerPreapplySuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::PreapplyPending { request, .. } => {
                        let header = BlockHeaderBuilder::default()
                            .level(content.response.level)
                            .proto(content.response.proto)
                            .predecessor(content.response.predecessor.clone())
                            .timestamp(content.response.timestamp)
                            .validation_pass(content.response.validation_pass)
                            .operations_hash(content.response.operations_hash.clone())
                            .fitness(content.response.fitness.clone())
                            .context(content.response.context.clone())
                            .protocol_data(request.protocol_data.clone().into())
                            .build()
                            .unwrap(); // Should be Infallible.

                        baker.block_baker = BakerBlockBakerState::PreapplySuccess {
                            time: action.time_as_nanos(),
                            header,
                            operations: request.operations.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerComputeProofOfWorkPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::PreapplySuccess {
                        header, operations, ..
                    } => {
                        baker.block_baker = BakerBlockBakerState::ComputeProofOfWorkPending {
                            time: action.time_as_nanos(),
                            req_id: content.req_id.clone(),
                            header: header.clone(),
                            operations: operations.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerComputeProofOfWorkSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &mut baker.block_baker {
                    BakerBlockBakerState::ComputeProofOfWorkPending {
                        header, operations, ..
                    } => {
                        header.set_proof_of_work_nonce(&content.proof_of_work_nonce);
                        baker.block_baker = BakerBlockBakerState::ComputeProofOfWorkSuccess {
                            time: action.time_as_nanos(),
                            header: header.clone(),
                            operations: operations.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerSignPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &mut baker.block_baker {
                    BakerBlockBakerState::ComputeProofOfWorkSuccess {
                        header, operations, ..
                    } => {
                        baker.block_baker = BakerBlockBakerState::SignPending {
                            time: action.time_as_nanos(),
                            req_id: content.req_id,
                            header: header.clone(),
                            operations: operations.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerSignSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &mut baker.block_baker {
                    BakerBlockBakerState::SignPending {
                        header, operations, ..
                    } => {
                        header.set_signature(&content.signature);
                        baker.block_baker = BakerBlockBakerState::SignSuccess {
                            time: action.time_as_nanos(),
                            header: header.clone(),
                            operations: operations.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerComputeOperationsPathsPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::SignSuccess {
                        header, operations, ..
                    } => {
                        baker.block_baker = BakerBlockBakerState::ComputeOperationsPathsPending {
                            time: action.time_as_nanos(),
                            protocol_req_id: content.protocol_req_id,
                            header: header.clone(),
                            operations: operations.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerComputeOperationsPathsSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::ComputeOperationsPathsPending {
                        header,
                        operations,
                        ..
                    } => {
                        baker.block_baker = BakerBlockBakerState::ComputeOperationsPathsSuccess {
                            time: action.time_as_nanos(),
                            header: header.clone(),
                            operations: operations.clone(),
                            operations_paths: content.operations_paths.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerInjectPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::ComputeOperationsPathsSuccess {
                        operations,
                        operations_paths,
                        ..
                    } => {
                        baker.block_baker = BakerBlockBakerState::InjectPending {
                            time: action.time_as_nanos(),
                            block: content.block.clone(),
                            operations: operations.clone(),
                            operations_paths: operations_paths.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerInjectSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::InjectPending { block, .. } => {
                        baker.block_baker = BakerBlockBakerState::InjectSuccess {
                            time: action.time_as_nanos(),
                            block: block.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn calc_seconds_until_round(
    current_round: u64,
    target_round: u64,
    min_block_delay: u64,
    delay_increment_per_round: u64,
) -> u64 {
    let rounds_left = target_round.saturating_sub(current_round);
    min_block_delay * rounds_left
        + delay_increment_per_round * rounds_left * (current_round + target_round).saturating_sub(1)
            / 2
}

fn calc_time_until_round(
    current_round: u64,
    target_round: u64,
    min_block_delay: u64,
    delay_increment_per_round: u64,
) -> u64 {
    calc_seconds_until_round(
        current_round,
        target_round,
        min_block_delay,
        delay_increment_per_round,
    ) * 1_000_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_seconds_until_round() {
        assert_eq!(calc_seconds_until_round(0, 0, 15, 5), 0);
        assert_eq!(calc_seconds_until_round(0, 1, 15, 5), 15);
        assert_eq!(calc_seconds_until_round(0, 2, 15, 5), 35);
        assert_eq!(calc_seconds_until_round(0, 3, 15, 5), 60);
        assert_eq!(calc_seconds_until_round(0, 4, 15, 5), 90);
        assert_eq!(calc_seconds_until_round(0, 5, 15, 5), 125);

        assert_eq!(calc_seconds_until_round(1, 2, 15, 5), 20);
        assert_eq!(calc_seconds_until_round(1, 3, 15, 5), 45);
        assert_eq!(calc_seconds_until_round(1, 4, 15, 5), 75);
        assert_eq!(calc_seconds_until_round(1, 5, 15, 5), 110);

        assert_eq!(calc_seconds_until_round(2, 3, 15, 5), 25);
        assert_eq!(calc_seconds_until_round(2, 4, 15, 5), 55);
        assert_eq!(calc_seconds_until_round(2, 5, 15, 5), 90);

        assert_eq!(calc_seconds_until_round(3, 4, 15, 5), 30);
        assert_eq!(calc_seconds_until_round(3, 5, 15, 5), 65);

        assert_eq!(calc_seconds_until_round(4, 5, 15, 5), 35);
    }
}
