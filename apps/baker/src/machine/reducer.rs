// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, time::Duration};

use chrono::{DateTime, Utc};

use redux_rs::ActionWithMeta;

use tezos_messages::{
    p2p::binary_message::BinaryRead,
    protocol::proto_012::operation::{InlinedPreendorsementContents, Contents, InlinedPreendorsementVariant},
};

use crate::{rpc_client::Validator, types::ProtocolBlockHeader};

use super::{action::*, state::{State, Config, BlockData, PreendorsementUnsignedOperation}};

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
                    *state = State::Ready {
                        config: Config {
                            chain_id: chain_id.clone(),
                            quorum_size: (constants.consensus_committee_size / 3 + 1) as usize,
                            minimal_block_delay: Duration::from_secs(block_sec),
                            delay_increment_per_round: Duration::from_secs(round_sec),
                        },
                        predecessor_head_data: None,
                        current_head_data: None,
                    }
                }
                _ => (),
            }
        }
        // WARNING: for now it is incorrect, new head should not always replace old head
        // need to accumulate rounds and keep predecessor block
        Action::NewHeadSeen(NewHeadSeenAction { head }) => match state {
            State::Ready {
                current_head_data, ..
            } => {
                let protocol_data_bytes = hex::decode(&head.protocol_data).unwrap();
                let protocol_data = ProtocolBlockHeader::from_bytes(protocol_data_bytes).unwrap();

                *current_head_data = Some(BlockData {
                    predecessor: head.predecessor.clone(),
                    block_hash: head.hash.clone(),

                    slot: None,
                    validators: BTreeMap::new(),
                    level: head.level,
                    round: protocol_data.payload_round,
                    timestamp: head.timestamp.parse::<DateTime<Utc>>().unwrap(),
                    payload_hash: protocol_data.payload_hash,

                    seen_preendorsement: 0,
                    preendorsement: None,
                    endorsement: None,
                })
            }
            _ => return,
        },
        Action::GetSlotsSuccess(GetSlotsSuccessAction {
            validators,
            this_delegate,
        }) => match state {
            State::Ready {
                current_head_data: Some(head_data),
                ..
            } => {
                let mut validators_map = BTreeMap::new();
                for Validator {
                    delegate, slots, ..
                } in validators
                {
                    validators_map.insert(delegate.clone(), slots.clone());
                }
                head_data.slot = validators_map
                    .get(this_delegate)
                    .and_then(|v| v.first().cloned());
                head_data.validators = validators_map;
            }
            _ => return,
        },
        Action::SignPreendorsement(SignPreendorsementAction {}) => {
            let head_data = match state {
                State::Ready {
                    current_head_data: Some(v),
                    ..
                } if !v.slot.is_none() => v,
                _ => return,
            };

            let inlined = InlinedPreendorsementVariant {
                slot: head_data.slot.unwrap(),
                level: head_data.level,
                round: head_data.round,
                block_payload_hash: head_data.payload_hash.clone(),
            };
            head_data.preendorsement = Some(PreendorsementUnsignedOperation {
                branch: head_data.predecessor.clone(),
                content: InlinedPreendorsementContents::Preendorsement(inlined),
            });
        }
        Action::NewOperationSeen(NewOperationSeenAction { operations }) => {
            let block_data = match state {
                State::Ready {
                    current_head_data: Some(block_data),
                    ..
                } => block_data,
                _ => return,
            };
            let validators = block_data.validators.clone();

            for operation in operations {
                for content in &operation.contents {
                    if let Contents::Preendorsement(preendorsement) = content {
                        let mut power = 0;
                        for slots in validators.values() {
                            if slots.contains(&preendorsement.slot) {
                                power = slots.len();
                            }
                        }
                        block_data.seen_preendorsement += power;
                    }
                }
            }
        }
        Action::SignEndorsement(SignEndorsementAction {}) => {
            let head_data = match state {
                State::Ready {
                    current_head_data: Some(v),
                    ..
                } if !v.slot.is_none() => v,
                _ => return,
            };

            let inlined = InlinedPreendorsementVariant {
                slot: head_data.slot.unwrap(),
                level: head_data.level,
                round: head_data.round,
                block_payload_hash: head_data.payload_hash.clone(),
            };
            head_data.preendorsement = Some(PreendorsementUnsignedOperation {
                branch: head_data.predecessor.clone(),
                content: InlinedPreendorsementContents::Preendorsement(inlined),
            });
        }
        _ => {}
    }
}
