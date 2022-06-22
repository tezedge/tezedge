// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;

use crypto::hash::{
    BlockPayloadHash, HashTrait, HashType, OperationListHash, OperationMetadataListListHash,
};
use storage::BlockHeaderWithHash;
use tezos_encoding::types::SizedBytes;
use tezos_messages::p2p::encoding::block_header::BlockHeaderBuilder;

use crate::baker::{BakerState, ElectedBlock};
use crate::block_applier::BlockApplierApplyState;
use crate::current_head::{CurrentHeadState, ProtocolConstants};
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
        Action::CurrentHeadRehydrated(_) | Action::CurrentHeadUpdate(_) | Action::BakerAdd(_) => {
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
                        let now = action.time_as_nanos();

                        let next_round =
                            RoundTimeoutIter::new_for_next_round(&state.current_head, slots)
                                .and_then(|mut iter| {
                                    iter.find(|baking_slot| baking_slot.timeout >= now)
                                });

                        let next_level = RoundTimeoutIter::new_for_next_level(
                            &state.current_head,
                            next_slots,
                            baker.elected_block_header_with_hash(),
                        )
                        .and_then(|mut iter| iter.find(|baking_slot| baking_slot.timeout >= now));

                        baker.block_baker = BakerBlockBakerState::TimeoutPending {
                            time: now,
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

struct RoundIter<'a> {
    slots: &'a [u16],
    committee_size: usize,
    slot_index: usize,
    slot_loop: usize,
}

impl<'a> RoundIter<'a> {
    fn new_for_current_round(round: u32, slots: &'a [u16], committee_size: u32) -> Option<Self> {
        if slots.is_empty() {
            return None;
        }
        let curr_slot = round.checked_rem(committee_size)?;
        let curr_slot_loop = round.checked_div(committee_size)?;
        let part = slots.partition_point(|&slot| u32::from(slot) <= curr_slot);
        let (slot_index, slot_loop) = if part < slots.len() {
            (part, curr_slot_loop)
        } else {
            (0, curr_slot_loop + 1)
        };
        Some(Self {
            slots,
            committee_size: committee_size.try_into().ok()?,
            slot_index,
            slot_loop: slot_loop.try_into().ok()?,
        })
    }

    fn new_for_first_round(slots: &'a [u16], committee_size: u32) -> Option<Self> {
        if slots.is_empty() {
            return None;
        }
        Some(Self {
            slots,
            committee_size: committee_size.try_into().ok()?,
            slot_index: 0,
            slot_loop: 0,
        })
    }

    fn round(&'a self) -> Option<u32> {
        let slot = self.slots.get(self.slot_index)?.clone().try_into().ok()?;
        Some(
            self.slot_loop
                .checked_mul(self.committee_size)?
                .checked_add(slot)?
                .try_into()
                .ok()?,
        )
    }

    fn next_slot_index_and_loop(&self) -> Option<(usize, usize)> {
        let slot_index = self.slot_index.checked_add(1)?;
        if slot_index < self.slots.len() {
            Some((slot_index, self.slot_loop))
        } else {
            Some((0, self.slot_loop.checked_add(1)?))
        }
    }
}

impl<'a> Iterator for RoundIter<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        let round = self.round()?;
        let (next_slot_index, next_slots_loop) = self.next_slot_index_and_loop()?;
        self.slot_index = next_slot_index;
        self.slot_loop = next_slots_loop;
        Some(round)
    }
}

struct RoundTimeoutIter<'a> {
    min_block_delay: u64,
    delay_increment_per_round: u64,

    round_iter: RoundIter<'a>,
    prev_round: u32,
    prev_timeout: u64,
}

impl<'a> RoundTimeoutIter<'a> {
    fn new_for_next_round_(
        prev_round: u32,
        prev_timeout: u64,
        slots: &'a [u16],
        consensus_committee_size: u32,
        min_block_delay: u64,
        delay_increment_per_round: u64,
    ) -> Option<Self> {
        let round_iter =
            RoundIter::new_for_current_round(prev_round, slots, consensus_committee_size)?;
        Some(RoundTimeoutIter {
            min_block_delay,
            delay_increment_per_round,
            round_iter,
            prev_round,
            prev_timeout,
        })
    }

    fn new_for_next_round(current_head: &'_ CurrentHeadState, slots: &'a [u16]) -> Option<Self> {
        let ProtocolConstants {
            min_block_delay,
            delay_increment_per_round,
            consensus_committee_size,
            ..
        } = current_head.constants()?;
        let pred = current_head.get()?;
        let prev_round = pred.header.fitness().round()?.try_into().ok()?;
        let prev_timeout: u64 = pred.header.timestamp().i64().try_into().ok()?;

        Self::new_for_next_round_(
            prev_round,
            prev_timeout,
            slots,
            *consensus_committee_size,
            *min_block_delay,
            *delay_increment_per_round,
        )
    }

    fn new_for_next_level_(
        prev_round: u32,
        prev_timeout: u64,
        slots: &'a [u16],
        consensus_committee_size: u32,
        min_block_delay: u64,
        delay_increment_per_round: u64,
    ) -> Option<Self> {
        let prev_timeout = calc_seconds_until_round(
            prev_round.into(),
            prev_round.checked_add(1)?.into(),
            min_block_delay,
            delay_increment_per_round,
        )
        .checked_add(prev_timeout)?;

        let round_iter = RoundIter::new_for_first_round(slots, consensus_committee_size)?;
        Some(RoundTimeoutIter {
            min_block_delay,
            delay_increment_per_round,
            round_iter,
            prev_round: 0,
            prev_timeout,
        })
    }

    fn new_for_next_level(
        current_head: &'_ CurrentHeadState,
        slots: &'a [u16],
        elected_block_header_with_hash: Option<&BlockHeaderWithHash>,
    ) -> Option<Self> {
        let ProtocolConstants {
            min_block_delay,
            delay_increment_per_round,
            consensus_committee_size,
            ..
        } = current_head.constants()?;
        let pred = elected_block_header_with_hash.or_else(|| current_head.get())?;
        let prev_round: u32 = pred
            .header
            .fitness()
            .round()
            .and_then(|round| round.try_into().ok())
            .unwrap_or(0);
        let prev_timeout: u64 = pred.header.timestamp().i64().try_into().ok()?;

        Self::new_for_next_level_(
            prev_round,
            prev_timeout,
            slots,
            *consensus_committee_size,
            *min_block_delay,
            *delay_increment_per_round,
        )
    }
}

impl<'a> Iterator for RoundTimeoutIter<'a> {
    type Item = BakingSlot;

    fn next(&mut self) -> Option<Self::Item> {
        let round = self.round_iter.next()?;
        let timeout = calc_seconds_until_round(
            self.prev_round.into(),
            round.into(),
            self.min_block_delay,
            self.delay_increment_per_round,
        )
        .checked_add(self.prev_timeout)?;

        self.prev_round = round;
        self.prev_timeout = timeout;

        Some(Self::Item {
            round,
            timeout: timeout.checked_mul(1_000_000_000)?,
        })
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

    #[test]
    fn test_rounds_iter_next_slot_and_loop() {
        let slots = &[0, 1, 2, 3];

        let default = RoundIter {
            slots,
            committee_size: 4,
            slot_index: 0,
            slot_loop: 0,
        };

        let iter = |slot_index, slot_loop| RoundIter {
            slot_index,
            slot_loop,
            ..default
        };

        assert_eq!(iter(0, 0).next_slot_index_and_loop(), Some((1, 0)));
        assert_eq!(iter(1, 0).next_slot_index_and_loop(), Some((2, 0)));
        assert_eq!(iter(2, 0).next_slot_index_and_loop(), Some((3, 0)));
        assert_eq!(iter(3, 0).next_slot_index_and_loop(), Some((0, 1)));
        assert_eq!(iter(0, 1).next_slot_index_and_loop(), Some((1, 1)));

        let slots = &[0];

        let default = RoundIter {
            slots,
            committee_size: 4,
            slot_index: 0,
            slot_loop: 0,
        };

        let iter = |slot_index, slot_loop| RoundIter {
            slot_index,
            slot_loop,
            ..default
        };

        assert_eq!(iter(0, 0).next_slot_index_and_loop(), Some((0, 1)));
        assert_eq!(iter(0, 1).next_slot_index_and_loop(), Some((0, 2)));
        assert_eq!(iter(0, 2).next_slot_index_and_loop(), Some((0, 3)));
        assert_eq!(iter(0, 3).next_slot_index_and_loop(), Some((0, 4)));
        assert_eq!(iter(0, 4).next_slot_index_and_loop(), Some((0, 5)));
    }

    #[test]
    fn test_rounds_iter_round() {
        let slots = &[0, 1, 2, 3];

        let default = RoundIter {
            slots,
            committee_size: 4,
            slot_index: 0,
            slot_loop: 0,
        };

        let iter = |slot_index, slot_loop| RoundIter {
            slot_index,
            slot_loop,
            ..default
        };

        assert_eq!(iter(0, 0).round(), Some(0));
        assert_eq!(iter(1, 0).round(), Some(1));
        assert_eq!(iter(2, 0).round(), Some(2));
        assert_eq!(iter(3, 0).round(), Some(3));
        assert_eq!(iter(0, 1).round(), Some(4));

        let slots = &[0, 2];

        let default = RoundIter {
            slots,
            committee_size: 4,
            slot_index: 0,
            slot_loop: 0,
        };

        let iter = |slot_index, slot_loop| RoundIter {
            slot_index,
            slot_loop,
            ..default
        };

        assert_eq!(iter(0, 0).round(), Some(0));
        assert_eq!(iter(1, 0).round(), Some(2));
        assert_eq!(iter(0, 1).round(), Some(4));
        assert_eq!(iter(1, 1).round(), Some(6));
        assert_eq!(iter(0, 2).round(), Some(8));
    }

    #[test]
    fn test_rounds_iter_new_for_current_round() {
        let slots = &[0, 1, 2, 3];

        assert_eq!(
            RoundIter::new_for_current_round(0, slots, 4)
                .unwrap()
                .next()
                .unwrap(),
            1
        );

        assert_eq!(
            RoundIter::new_for_current_round(3, slots, 4)
                .unwrap()
                .next()
                .unwrap(),
            4
        );

        let slots = &[0, 2];

        assert_eq!(
            RoundIter::new_for_current_round(0, slots, 4)
                .unwrap()
                .next()
                .unwrap(),
            2
        );

        assert_eq!(
            RoundIter::new_for_current_round(1, slots, 4)
                .unwrap()
                .next()
                .unwrap(),
            2
        );

        assert_eq!(
            RoundIter::new_for_current_round(2, slots, 4)
                .unwrap()
                .next()
                .unwrap(),
            4
        );

        assert_eq!(
            RoundIter::new_for_current_round(3, slots, 4)
                .unwrap()
                .next()
                .unwrap(),
            4
        );
    }

    #[test]
    fn test_next_round() {
        let min_block_delay = 15;
        let delay_increment_per_round = 5;
        let consensus_committee_size = 5;
        let slots_0_3 = [0, 3];

        let next_round_iter = |prev_round, prev_timeout, slots| {
            RoundTimeoutIter::new_for_next_round_(
                prev_round,
                prev_timeout,
                slots,
                consensus_committee_size,
                min_block_delay,
                delay_increment_per_round,
            )
            .unwrap()
        };

        let from_secs = |t: u64| t * 1_000_000_000;

        let mut iter = next_round_iter(0, 0, &slots_0_3);
        assert_eq!(
            iter.next(),
            Some(BakingSlot {
                round: 3,
                timeout: from_secs(15 + 20 + 25),
            })
        );
        assert_eq!(
            iter.next(),
            Some(BakingSlot {
                round: 5 + 0,
                timeout: from_secs(15 + 20 + 25 + 30 + 35),
            })
        );
        assert_eq!(
            iter.next(),
            Some(BakingSlot {
                round: 5 + 3,
                timeout: from_secs(15 + 20 + 25 + 30 + 35 + 40 + 45 + 50),
            })
        );
    }

    #[test]
    fn test_next_level() {
        let min_block_delay = 15;
        let delay_increment_per_round = 5;
        let consensus_committee_size = 5;
        let slots_with_first_0 = [0, 3];
        let slots_with_first_1 = [1, 3];

        let next_level_iter = |prev_round, prev_timeout, slots| {
            RoundTimeoutIter::new_for_next_level_(
                prev_round,
                prev_timeout,
                slots,
                consensus_committee_size,
                min_block_delay,
                delay_increment_per_round,
            )
            .unwrap()
        };

        let from_secs = |t: u64| t * 1_000_000_000;

        assert_eq!(
            next_level_iter(0, 0, &slots_with_first_0).next(),
            Some(BakingSlot {
                round: 0,
                timeout: from_secs(15) // 15 secs for current 0 round to bake
            })
        );
        assert_eq!(
            next_level_iter(1, 0, &slots_with_first_0).next(),
            Some(BakingSlot {
                round: 0,
                timeout: from_secs(15 + 5) // 15 + 5 secs for current 1 round to bake
            })
        );
        assert_eq!(
            next_level_iter(0, 0, &slots_with_first_1).next(),
            Some(BakingSlot {
                round: 1,
                timeout: from_secs(15 + 15) // 15 secs for current 0 round + 15 secs for next level round 0 to bake
            })
        );
        assert_eq!(
            next_level_iter(1, 0, &slots_with_first_1).next(),
            Some(BakingSlot {
                round: 1,
                timeout: from_secs(20 + 15) // 20 secs for current 1 round + 15 secs for next level round 0 to bake
            })
        );
    }
}
