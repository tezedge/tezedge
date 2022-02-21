// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Duration;

use redux_rs::ActionWithMeta;
use tezos_messages::protocol::proto_012::operation::{
    InlinedPreendorsementContents, InlinedPreendorsementVariant,
};

use crate::types::PreendorsementUnsignedOperation;

use super::{
    super::types::{
        BlockInfo, EndorsablePayload, LevelState, Phase, Prequorum,
        Proposal, RoundState,
    },
    action::*,
    state::{Config, State},
};

pub fn reducer(state: &mut State, action: &ActionWithMeta<Action>) {
    match &action.action {
        Action::GetChainIdSuccess(GetChainIdSuccessAction { chain_id }) => {
            *state = State::GotChainId(chain_id.clone());
        }
        Action::GetChainIdError(GetChainIdErrorAction { error }) => {
            *state = State::RpcError(error.to_string());
        }
        Action::GetConstantsSuccess(GetConstantsSuccessAction { constants }) => {
            let (block_sec, round_sec) = (
                constants.minimal_block_delay.parse::<u64>(),
                constants.delay_increment_per_round.parse::<u64>(),
            );
            let (block_sec, round_sec) = match (block_sec, round_sec) {
                (Ok(block_sec), Ok(round_sec)) => (block_sec, round_sec),
                _ => {
                    *state = State::ContextConstantsParseError;
                    return;
                }
            };
            match &*state {
                State::GotChainId(chain_id) => {
                    *state = State::GotConstants(Config {
                        chain_id: chain_id.clone(),
                        quorum_size: (constants.consensus_committee_size / 3 + 1) as usize,
                        minimal_block_delay: Duration::from_secs(block_sec),
                        delay_increment_per_round: Duration::from_secs(round_sec),
                    });
                }
                _ => (),
            }
        }
        Action::NewProposal(NewProposalAction {
            new_proposal,
            delegate_slots,
            next_level_delegate_slots,
            now_timestamp,
        }) => {
            match state {
                State::GotConstants(config) => {
                    *state = State::Ready {
                        config: config.clone(),
                        preendorsement: None,
                        endorsement: None,
                        level_state: LevelState {
                            current_level: new_proposal.block.level,
                            latest_proposal: new_proposal.clone(),
                            locked_round: None,
                            endorsable_payload: None,
                            elected_block: None,
                            delegate_slots: delegate_slots.clone(),
                            next_level_delegate_slots: next_level_delegate_slots.clone(),
                            next_level_proposed_round: None,
                        },
                        round_state: RoundState {
                            current_round: {
                                if new_proposal.block.protocol != new_proposal.block.next_protocol {
                                    // If our current proposal is the transition block, we suppose a
                                    // never ending round 0
                                    0
                                } else {
                                    round_by_timestamp(
                                        now_timestamp.0,
                                        &new_proposal.predecessor,
                                        &*config,
                                    )
                                }
                            },
                            current_phase: Phase::NonProposer,
                        },
                    }
                }
                State::Ready {
                    config,
                    level_state,
                    round_state,
                    preendorsement,
                    ..
                } => {
                    let new_proposal_level = new_proposal.block.level;
                    let new_proposal_round = new_proposal.block.round;

                    if level_state.current_level < new_proposal_level {
                        // Possible scenarios:
                        //    - we received a block for a next level
                        //    - we received our own block
                        //      This is where we update our [level_state] (and our [round_state])
                        *level_state = LevelState {
                            current_level: new_proposal_level,
                            latest_proposal: new_proposal.clone(),
                            locked_round: None,
                            endorsable_payload: None,
                            elected_block: None,
                            delegate_slots: delegate_slots.clone(),
                            next_level_delegate_slots: next_level_delegate_slots.clone(),
                            next_level_proposed_round: None,
                        };
                        *round_state = RoundState {
                            current_round: {
                                if new_proposal.block.protocol != new_proposal.block.next_protocol {
                                    // If our current proposal is the transition block, we suppose a
                                    // never ending round 0
                                    0
                                } else {
                                    round_by_timestamp(
                                        now_timestamp.0,
                                        &new_proposal.predecessor,
                                        &*config,
                                    )
                                }
                            },
                            current_phase: Phase::NonProposer,
                        };
                    }

                    if level_state.current_level > new_proposal_level {
                        // The baker is ahead, a reorg may have happened. Do nothing:
                        // wait for the node to send us the branch's head. This new head
                        // should have a fitness that is greater than our current
                        // proposal and thus, its level should be at least the same as
                        // our current proposal's level.
                    } else if level_state.current_level == new_proposal_level {
                        // The received head is a new proposal for the current level:
                        // let's check if it's a valid one for us.
                        if level_state.latest_proposal.predecessor.hash != new_proposal.predecessor.hash {
                            // new_proposal_is_on_another_branch
                            let switch = match (&level_state.endorsable_payload, &new_proposal.block.prequorum) {
                                (None, _) => {
                                    // The new branch contains a PQC (and we do not) or a better
                                    // fitness, we switch.
                                    true
                                }
                                (Some(_), None) => {
                                    // We have a better PQC, we don't switch as we are able to
                                    // propose a better chain if we stay on our current one.
                                    false
                                }
                                (Some(EndorsablePayload { prequorum: current_pqc, .. } ), Some(new_pqc)) => {
                                    if current_pqc.round > new_pqc.round {
                                        // The other's branch PQC is lower than ours, do not switch
                                        false
                                    } else if current_pqc.round < new_pqc.round {
                                        // Their PQC is better than ours: we switch
                                        true
                                    } else {
                                        // `current_pqc.round < new_pqc.round`
                                        // There is a PQC on two branches with the same round and
                                        // the same level but not the same predecessor : it's
                                        // impossible unless if there was some double-baking. This
                                        // shouldn't happen but do nothing anyway.
                                        false
                                    }
                                }
                            };
                            if switch {
                                level_state.latest_proposal = new_proposal.clone();
                                *round_state = RoundState {
                                    current_round: round_by_timestamp(
                                        now_timestamp.0,
                                        &new_proposal.predecessor,
                                        &*config,
                                    ),
                                    current_phase: Phase::NonProposer,
                                };
                            } else {
                                return;
                            }
                        }

                        let current_round = round_state.current_round;
                        if current_round < new_proposal_round {
                            // The proposal is invalid: we ignore it.
                        } else {
                            if current_round == new_proposal_round
                                && new_proposal.block.round == level_state.latest_proposal.block.round
                                && new_proposal.block.hash != level_state.latest_proposal.block.hash
                                && new_proposal.predecessor.hash == level_state.latest_proposal.predecessor.hash
                            {
                                // An existing proposal was found at the same round: the
                                // proposal is bad and should be punished by the accuser
                                return;
                            }

                            // Check whether we need to update our endorsable payload.
                            may_update_endorsable_payload_with_internal_pqc(level_state, new_proposal);
                            assert_eq!(level_state.latest_proposal.block.level, new_proposal.block.level);

                            if level_state.latest_proposal.block.round < new_proposal_round {
                                // updating_latest_proposal
                                level_state.latest_proposal = new_proposal.clone();
                            }

                            if current_round == new_proposal_round {
                                // Valid proposal.
                                let need_preendorse = match &level_state.locked_round {
                                    Some(locked_round) => {
                                        if locked_round.payload_hash == new_proposal.block.payload_hash {
                                            true
                                        } else {
                                            match &new_proposal.block.prequorum {
                                                Some(Prequorum { round, .. }) => locked_round.round < *round,
                                                // do nothing, should not preendorse
                                                _ => false,
                                            }
                                        }
                                    }
                                    None => true,
                                };
                                if need_preendorse {
                                    if new_proposal.block.protocol == new_proposal.block.next_protocol {
                                        if let Some(slot) = delegate_slots.slot {
                                            let inlined = InlinedPreendorsementVariant {
                                                slot,
                                                level: new_proposal.block.level,
                                                round: new_proposal.block.round,
                                                block_payload_hash: new_proposal
                                                    .block
                                                    .payload_hash
                                                    .clone(),
                                            };
                                            *preendorsement = Some(PreendorsementUnsignedOperation {
                                                branch: level_state.latest_proposal.predecessor.hash.clone(),
                                                content: InlinedPreendorsementContents::Preendorsement(inlined),
                                            });
                                        }
                                        round_state.current_phase = Phase::CollectingPreendorsements;
                                    } else {
                                        round_state.current_phase = Phase::NonProposer;
                                    }
                                }
                            } else {
                                // `current_round > new_proposal_round`
                                // Outdated proposal.
                            }
                        }
                    }
                }
                _ => return,
            }
        }
        Action::NewOperationSeen(NewOperationSeenAction { operations }) => {
            let _ = operations;
            // for operation in operations {
            //     for content in &operation.contents {
            //         if let Contents::Preendorsement(preendorsement) = content {
            //             let mut power = 0;
            //             for slots in validators.values() {
            //                 if slots.contains(&preendorsement.slot) {
            //                     power = slots.len();
            //                 }
            //             }
            //             block_data.seen_preendorsement += power;
            //         }
            //     }
            // }
        }
        _ => {}
    }
}

fn may_update_endorsable_payload_with_internal_pqc(
    level_state: &mut LevelState,
    new_proposal: &Proposal,
) {
    match (&new_proposal.block.prequorum, &level_state.endorsable_payload) {
        // The proposal does not contain a PQC: no need to update
        (None, _) => (),
        (
            Some(Prequorum { round: new_round, .. }),
            Some(EndorsablePayload { prequorum: Prequorum { round: old_round, .. }, .. }),
        ) if new_round < old_round => (), // The proposal pqc is outdated, do not update
        (Some(better_prequorum), _) => {
            assert_eq!(better_prequorum.payload_hash, new_proposal.block.payload_hash);
            assert_eq!(better_prequorum.level, new_proposal.block.level);
            level_state.endorsable_payload = Some(EndorsablePayload {
                proposal: new_proposal.clone(),
                prequorum: better_prequorum.clone(),
            });
        }
    }
}

fn round_by_timestamp(now_timestamp: u64, predecessor: &BlockInfo, config: &Config) -> i32 {
    let pred_round = predecessor.round as u32;
    let pred_time = predecessor.timestamp as u64;
    let last_round_duration =
        config.minimal_block_delay + config.delay_increment_per_round * pred_round;
    let last_round_duration = last_round_duration.as_secs();
    let start_of_current_level = pred_time + last_round_duration;
    if now_timestamp < start_of_current_level {
        // receive proposal from the future
        i32::MIN
    } else {
        let elapsed = now_timestamp - start_of_current_level;
        // m := minimal_block_delay
        // d := delay_increment_per_round
        // r := round
        // e := elapsed
        // duration(r) = m + d * r
        // e = duration(0) + duration(1) + ... + duration(r - 1)
        // e = m + (m + d) + (m + d * 2) + ... + (m + d * (r - 1))
        // e = m * r + d * r * (r - 1) / 2
        // d * r^2 + (2 * m - d) * r - 2 * e = 0
        let e = elapsed as f64;
        let d = config.delay_increment_per_round.as_secs() as f64;
        let m = config.minimal_block_delay.as_secs() as f64;
        let p = d - 2.0 * m;
        let r = (p + (p * p + 8.0 * d * e).sqrt()) / (2.0 * d);

        r.floor() as i32
    }
}
