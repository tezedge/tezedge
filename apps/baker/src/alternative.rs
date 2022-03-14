// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, convert::TryInto, sync::mpsc::Receiver, time::Duration};

use super::{
    machine::{
        action::{Action, *},
        service::ServiceDefault,
    },
    proof_of_work::guess_proof_of_work,
    types::{BlockInfo, BlockPayload, Mempool, Prequorum, ProtocolBlockHeader, Slots, Timestamp},
};

use crypto::{
    blake2b,
    hash::{
        BlockHash, BlockPayloadHash, ChainId, ContractTz1Hash, NonceHash, OperationListHash,
        Signature,
    },
};
use tenderbake::{BlockId, Config, Machine, Validator, ValidatorMap};
use tezos_messages::protocol::proto_012::operation::{
    Contents, InlinedEndorsement, Operation, InlinedEndorsementMempoolContentsEndorsementVariant, InlinedEndorsementMempoolContents, InlinedPreendorsement, InlinedPreendorsementContents, InlinedPreendorsementVariant,
};

impl From<Timestamp> for tenderbake::Timestamp {
    fn from(Timestamp(unix_epoch): Timestamp) -> Self {
        tenderbake::Timestamp { unix_epoch }
    }
}

impl From<tenderbake::Timestamp> for Timestamp {
    fn from(tenderbake::Timestamp { unix_epoch }: tenderbake::Timestamp) -> Self {
        Timestamp(unix_epoch)
    }
}

impl tenderbake::Payload for BlockPayload {
    type Item = Operation;

    const EMPTY: Self = BlockPayload {
        votes_payload: vec![],
        anonymous_payload: vec![],
        managers_payload: vec![],
    };

    fn update(&mut self, item: Self::Item) {
        for content in &item.contents {
            match content {
                Contents::Preendorsement(_)
                | Contents::Endorsement(_)
                | Contents::FailingNoop(_) => break,
                Contents::Proposals(_) | Contents::Ballot(_) => {
                    self.votes_payload.push(item);
                    break;
                }
                Contents::SeedNonceRevelation(_)
                | Contents::DoublePreendorsementEvidence(_)
                | Contents::DoubleEndorsementEvidence(_)
                | Contents::DoubleBakingEvidence(_)
                | Contents::ActivateAccount(_) => {
                    self.anonymous_payload.push(item);
                    break;
                }
                _ => {
                    self.managers_payload.push(item);
                    break;
                }
            }
        }
    }
}

struct SlotsInfo {
    this: ContractTz1Hash,
    validators: BTreeMap<ContractTz1Hash, u32>,
    validators_rev: BTreeMap<u32, ContractTz1Hash>,
    counter: u32,
    delegates: BTreeMap<i32, BTreeMap<ContractTz1Hash, Slots>>,
}

impl SlotsInfo {
    fn populate(&mut self, level: i32, delegates: BTreeMap<ContractTz1Hash, Slots>) {
        self.delegates.insert(level, delegates.clone());
        let SlotsInfo {
            validators,
            counter,
            validators_rev,
            ..
        } = self;
        for (validator, _) in delegates {
            validators.entry(validator.clone()).or_insert_with(|| {
                let id = *counter;
                validators_rev.insert(id, validator);
                *counter += 1;
                id
            });
        }
    }

    fn slots(&self, level: i32) -> Option<&Vec<u16>> {
        self.delegates.get(&level)?.get(&self.this).map(|s| &s.0)
    }

    fn slot(&self, id: u32, level: i32) -> Option<u16> {
        self.delegates
            .get(&level)?
            .get(self.validators_rev.get(&id)?)?
            .0
            .first()
            .cloned()
    }

    fn id(&self) -> Option<u32> {
        self.validators.get(&self.this).cloned()
    }

    fn validator(&self, level: i32, slot: u16) -> Option<Validator> {
        let i = self.delegates.get(&level)?;
        let (k, s) = i.iter().find(|&(_, v)| v.0.first() == Some(&slot))?;
        let id = self.validators.get(k)?;
        Some(Validator {
            id: *id,
            power: s.0.len() as u32,
        })
    }

    fn convert_prequorum(&self, v: Prequorum) -> tenderbake::Prequorum<Operation> {
        let level = v.level;
        tenderbake::Prequorum {
            block_id: BlockId {
                level,
                round: v.round,
                payload_hash: v.payload_hash.0.as_slice().try_into().unwrap(),
                payload_round: v.round,
            },
            votes: {
                v.firsts_slot
                    .into_iter()
                    .zip(v.ops.into_iter())
                    .filter_map(|(slot, op)| {
                        let i = self.delegates.get(&level)?;
                        let (k, s) = i.iter().find(|&(_, v)| v.0.first() == Some(&slot))?;
                        let id = self.validators.get(k)?;
                        Some((
                            Validator {
                                id: *id,
                                power: s.0.len() as u32,
                            },
                            op,
                        ))
                    })
                    .collect()
            },
        }
    }

    fn convert_quorum(
        &self,
        v: &[(InlinedEndorsementMempoolContentsEndorsementVariant, Operation)],
    ) -> tenderbake::Quorum<Operation> {
        tenderbake::Quorum {
            votes: v
                .iter()
                .filter_map(|(v, op)| {
                    let i = self.delegates.get(&v.level)?;
                    let (k, s) = i.iter().find(|&(_, s)| s.0.first() == Some(&v.slot))?;
                    let id = self.validators.get(k)?;
                    Some((
                        Validator {
                            id: *id,
                            power: s.0.len() as u32,
                        },
                        op.clone(),
                    ))
                })
                .collect(),
        }
    }

    fn convert_block_info(&self, v: BlockInfo) -> tenderbake::BlockInfo<BlockPayload> {
        tenderbake::BlockInfo {
            pred_hash: v.predecessor.0.as_slice().try_into().unwrap(),
            hash: v.hash.0.as_slice().try_into().unwrap(),
            block_id: BlockId {
                level: v.level,
                round: v.round,
                payload_hash: v.payload_hash.0.as_slice().try_into().unwrap(),
                payload_round: v.payload_round,
            },
            timestamp: Timestamp(Duration::from_secs(v.timestamp as u64)).into(),
            transition: v.protocol != v.next_protocol,
            prequorum: v.prequorum.map(|v| self.convert_prequorum(v)),
            quorum: {
                let q = self.convert_quorum(&v.quorum);
                if q.votes.is_empty() {
                    None
                } else {
                    Some(q)
                }
            },
            payload: BlockPayload {
                votes_payload: v.payload.votes_payload,
                anonymous_payload: v.payload.anonymous_payload,
                managers_payload: v.payload.managers_payload,
            },
        }
    }
}

impl ValidatorMap for SlotsInfo {
    fn preendorser(&self, level: i32, round: i32) -> Option<Validator> {
        let _ = round;
        let id = self.id()?;
        let slots = self.slots(level)?;
        Some(Validator {
            id,
            power: slots.len() as u32,
        })
    }

    fn endorser(&self, level: i32, round: i32) -> Option<Validator> {
        let _ = round;
        let id = self.id()?;
        let slots = self.slots(level)?;
        Some(Validator {
            id,
            power: slots.len() as u32,
        })
    }

    fn proposer(&self, level: i32, round: i32) -> Option<i32> {
        self.slots(level)
            .into_iter()
            .flatten()
            .skip_while(|c| **c < round as u16)
            .next()
            .map(|r| *r as i32)
    }
}

pub fn run(service: &mut ServiceDefault, events: &mut Receiver<Action>) {
    let env = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .try_init()
        .unwrap();

    let chain_id = service.client.get_chain_id().unwrap();
    let constants = service.client.get_constants().unwrap();

    slog::info!(
        &service.logger,
        "committee size: {}",
        constants.consensus_committee_size
    );
    let config = Config {
        consensus_threshold: 2 * (constants.consensus_committee_size / 3) + 1,
        minimal_block_delay: Duration::from_secs(constants.minimal_block_delay.parse().unwrap()),
        delay_increment_per_round: Duration::from_secs(
            constants.delay_increment_per_round.parse().unwrap(),
        ),
    };
    let proof_of_work_threshold = constants.proof_of_work_threshold.parse::<i64>().unwrap();
    let mut state = Machine::empty();

    let mut slots_info = SlotsInfo {
        this: service.crypto.public_key_hash().clone(),
        validators: BTreeMap::new(),
        counter: 0,
        validators_rev: BTreeMap::new(),
        delegates: BTreeMap::new(),
    };

    service
        .client
        .monitor_proposals(
            &chain_id,
            service.crypto.public_key_hash().clone(),
            // it is first time we listening proposals,
            // we know nothing, so no timeout required
            i64::MAX,
            Action::Timeout,
            Action::NewProposal,
        )
        .unwrap();

    while let Ok(action) = events.recv() {
        let tb_actions = match action {
            Action::Timeout(TimeoutAction { .. }) => {
                let event = tenderbake::Event::Timeout;
                state
                    .handle(&config, &slots_info, event)
                    .into_iter()
                    .collect::<Vec<_>>()
            }
            Action::NewProposal(NewProposalAction {
                new_proposal,
                delegate_slots,
                next_level_delegate_slots,
                now_timestamp,
            }) => {
                slots_info.populate(delegate_slots.level, delegate_slots.delegates);
                slots_info.populate(
                    next_level_delegate_slots.level,
                    next_level_delegate_slots.delegates,
                );
                let pred = slots_info.convert_block_info(new_proposal.predecessor);
                let event = tenderbake::Event::Proposal(
                    Box::new(tenderbake::Proposal {
                        pred_timestamp: pred.timestamp,
                        pred_round: pred.block_id.round,
                        head: slots_info.convert_block_info(new_proposal.block),
                    }),
                    now_timestamp.into(),
                );
                if let Err(err) = service.client.monitor_operations(
                    i64::MAX,
                    Action::Timeout,
                    Action::NewOperationSeen,
                ) {
                    slog::error!(service.logger, "{:?}", err);
                }
                state
                    .handle(&config, &slots_info, event)
                    .into_iter()
                    .collect()
            }
            Action::NewOperationSeen(NewOperationSeenAction { operations }) => {
                let mut tb_actions = vec![];
                for op in operations {
                    for content in &op.contents {
                        match content {
                            Contents::Preendorsement(v) => {
                                if let Some(validator) = slots_info.validator(v.level, v.slot) {
                                    let event = tenderbake::Event::Preendorsement(
                                        tenderbake::Preendorsement {
                                            validator,
                                            block_id: BlockId {
                                                level: v.level,
                                                round: v.round,
                                                payload_hash: v
                                                    .block_payload_hash
                                                    .0
                                                    .as_slice()
                                                    .try_into()
                                                    .unwrap(),
                                                payload_round: v.round,
                                            },
                                        },
                                        op.clone(),
                                    );
                                    tb_actions.extend(
                                        state.handle(&config, &slots_info, event).into_iter(),
                                    );
                                }
                            }
                            Contents::Endorsement(v) => {
                                if let Some(validator) = slots_info.validator(v.level, v.slot) {
                                    let event = tenderbake::Event::Endorsement(
                                        tenderbake::Endorsement {
                                            validator,
                                            block_id: BlockId {
                                                level: v.level,
                                                round: v.round,
                                                payload_hash: v
                                                    .block_payload_hash
                                                    .0
                                                    .as_slice()
                                                    .try_into()
                                                    .unwrap(),
                                                payload_round: v.round,
                                            },
                                        },
                                        op.clone(),
                                        Timestamp::now().into(),
                                    );
                                    tb_actions.extend(
                                        state.handle(&config, &slots_info, event).into_iter(),
                                    );
                                }
                            }
                            _ => break,
                        }
                    }
                    let event = tenderbake::Event::PayloadItem(op);
                    tb_actions.extend(state.handle(&config, &slots_info, event).into_iter());
                }
                tb_actions
            }
            Action::PreapplyBlockSuccess(PreapplyBlockSuccessAction {
                mut header,
                operations,
            }) => {
                header.signature.0 = vec![0x00; 64];
                guess_proof_of_work(&mut header, proof_of_work_threshold);
                slog::info!(service.logger, "{:?}", header);
                header.signature.0.clear();
                let (data, _) = service.crypto.sign(0x11, &chain_id, &header).unwrap();
                service
                    .client
                    .inject_block(
                        data,
                        operations.clone(),
                        i64::MAX,
                        Action::Timeout,
                        |hash| InjectBlockSuccessAction { hash }.into(),
                    )
                    .unwrap();
                vec![]
            }
            Action::UnrecoverableError(UnrecoverableErrorAction { rpc_error }) => {
                slog::error!(service.logger, "{rpc_error}");
                vec![]
            }
            _ => vec![],
        };
        for action in tb_actions {
            perform(
                &chain_id,
                &slots_info,
                constants.blocks_per_commitment,
                service,
                action,
            );
        }
    }
}

fn perform(
    chain_id: &ChainId,
    slots_info: &SlotsInfo,
    blocks_per_commitment: u32,
    service: &mut ServiceDefault,
    action: tenderbake::Action<BlockPayload>,
) {
    match action {
        tenderbake::Action::ScheduleTimeout(timestamp) => {
            service.timer.timeout(timestamp.into(), Action::Timeout);
        }
        tenderbake::Action::Propose(block) => {
            let mempool = Mempool {
                endorsements: vec![],
                preendorsements: vec![],
                consensus_payload: block
                    .quorum
                    .map(|q| q.votes.ids)
                    .into_iter()
                    .flatten()
                    .map(|(_, op)| op)
                    .collect(),
                preendorsement_consensus_payload: block
                    .prequorum
                    .map(|q| q.votes.ids)
                    .into_iter()
                    .flatten()
                    .map(|(_, op)| op)
                    .collect(),
                payload: BlockPayload {
                    votes_payload: block.payload.votes_payload,
                    anonymous_payload: block.payload.anonymous_payload,
                    managers_payload: block.payload.managers_payload,
                },
            };
            let votes_ops = mempool.payload.votes_payload.iter();
            let anonymous_ops = mempool.payload.anonymous_payload.iter();
            let managers_ops = mempool.payload.managers_payload.iter();
            let ops = votes_ops.chain(anonymous_ops).chain(managers_ops);
            let hashes = ops
                .map(|op| op.hash.as_ref().cloned().unwrap())
                .collect::<Vec<_>>();
            let operation_list_hash = OperationListHash::calculate(&hashes).unwrap();
            let payload_round = block.block_id.payload_round;
            let predecessor_hash = BlockHash(block.pred_hash.to_vec());
            let payload_hash = if block.block_id.payload_hash == [0; 32] {
                BlockPayloadHash::calculate(
                    &predecessor_hash,
                    payload_round as u32,
                    &operation_list_hash,
                )
                .unwrap()
            } else {
                BlockPayloadHash(block.block_id.payload_hash.to_vec())
            };
            let pos_in_cycle = (block.block_id.level as u32) % blocks_per_commitment;
            let mut protocol_block_header = ProtocolBlockHeader {
                payload_hash,
                payload_round,
                seed_nonce_hash: if pos_in_cycle == 0 {
                    Some(NonceHash(blake2b::digest_256(&[1, 2, 3]).unwrap()))
                } else {
                    None
                },
                proof_of_work_nonce: hex::decode("7985fafe1fb70300").unwrap(),
                liquidity_baking_escape_vote: false,
                signature: Signature(vec![]),
            };
            let (_, signature) = service
                .crypto
                .sign(0x11, chain_id, &protocol_block_header)
                .unwrap();
            protocol_block_header.signature = signature;
            service
                .client
                .preapply_block(
                    protocol_block_header,
                    mempool,
                    Some(predecessor_hash),
                    block.timestamp.unix_epoch.as_secs() as i64,
                    i64::MAX,
                    Action::Timeout,
                    |header, operations| PreapplyBlockSuccessAction { header, operations }.into(),
                )
                .unwrap();
        }
        tenderbake::Action::Preendorse { pred_hash, content } => {
            let preendorsement = InlinedPreendorsement {
                branch: BlockHash(pred_hash.to_vec()),
                operations: InlinedPreendorsementContents::Preendorsement(InlinedPreendorsementVariant {
                    slot: slots_info
                        .slot(content.validator.id, content.block_id.level)
                        .unwrap(),
                    level: content.block_id.level,
                    round: content.block_id.round,
                    block_payload_hash: BlockPayloadHash(content.block_id.payload_hash.to_vec()),
                }),
                signature: Signature(vec![]),
            };
            let (data, _) = service.crypto.sign(0x12, chain_id, &preendorsement).unwrap();
            let op = &hex::encode(data);
            service
                .client
                .inject_operation(chain_id, &op, i64::MAX, Action::Timeout, |hash| {
                    InjectEndorsementSuccessAction { hash }.into()
                })
                .unwrap();
        }
        tenderbake::Action::Endorse { pred_hash, content } => {
            let endorsement = InlinedEndorsement {
                branch: BlockHash(pred_hash.to_vec()),
                operations: InlinedEndorsementMempoolContents::Endorsement(InlinedEndorsementMempoolContentsEndorsementVariant {
                    slot: slots_info
                        .slot(content.validator.id, content.block_id.level)
                        .unwrap(),
                    level: content.block_id.level,
                    round: content.block_id.round,
                    block_payload_hash: BlockPayloadHash(content.block_id.payload_hash.to_vec()),
                }),
                signature: Signature(vec![]),
            };
            let (data, _) = service.crypto.sign(0x13, chain_id, &endorsement).unwrap();
            let op = &hex::encode(data);
            service
                .client
                .inject_operation(chain_id, &op, i64::MAX, Action::Timeout, |hash| {
                    InjectEndorsementSuccessAction { hash }.into()
                })
                .unwrap();
        }
    }
}
