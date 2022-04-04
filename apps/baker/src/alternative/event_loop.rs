// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use slog::Logger;

use crypto::hash::{
    BlockHash, BlockPayloadHash, ChainId, ContractTz1Hash, OperationListHash, Signature,
};
use tenderbake as tb;
use tezos_encoding::{enc::BinWriter, types::SizedBytes};
use tezos_messages::protocol::proto_012::operation::{
    InlinedEndorsement, InlinedEndorsementMempoolContents,
    InlinedEndorsementMempoolContentsEndorsementVariant, InlinedPreendorsement,
    InlinedPreendorsementContents, InlinedPreendorsementVariant,
};

use crate::services::{
    client::{ProtocolBlockHeader, RpcClient, RpcError},
    event::{Event, OperationKind, OperationSimple},
    key::CryptoService,
    timer::Timer,
    Services,
};

use super::{guess_proof_of_work, seed_nonce::SeedNonceService, slots_info::SlotsInfo};

const WAIT_OPERATION_TIMEOUT: Duration = Duration::from_secs(3600);

pub fn run(
    srv: Services,
    base_dir: &PathBuf,
    baker: &str,
    events: impl Iterator<Item = Result<Event, RpcError>>,
) -> Result<(), RpcError> {
    let chain_id = srv.client.get_chain_id()?;
    srv.client.wait_bootstrapped()?;

    let constants = srv.client.get_constants()?;
    let mut seed_nonce = SeedNonceService::new(
        &base_dir,
        baker,
        constants.blocks_per_commitment,
        constants.blocks_per_cycle,
        constants.nonce_length,
    )
    .unwrap();

    slog::info!(
        srv.log,
        "committee size: {}",
        constants.consensus_committee_size
    );
    slog::info!(
        srv.log,
        "pow threshold: {:x}",
        constants.proof_of_work_threshold
    );
    let timing = tb::TimingLinearGrow {
        minimal_block_delay: constants.minimal_block_delay,
        delay_increment_per_round: constants.delay_increment_per_round,
    };
    let proof_of_work_threshold = constants.proof_of_work_threshold;

    srv.client.monitor_heads(&chain_id)?;

    let ours = vec![srv.crypto.public_key_hash().clone()];
    let mut dy = tb::Config {
        timing,
        map: SlotsInfo::new(constants.consensus_committee_size, ours),
        quorum: 2 * (constants.consensus_committee_size / 3) + 1,
    };
    let mut state = tb::Machine::<ContractTz1Hash, OperationSimple, 200>::default();

    let mut ahead_ops = BTreeMap::<BlockHash, Vec<OperationSimple>>::new();
    let mut live_blocks = vec![];
    let mut this_level = BTreeSet::new();

    for event in events {
        let unix_epoch = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let now = tb::Timestamp { unix_epoch };
        let actions = match event {
            Err(err) => {
                slog::error!(srv.log, " .  {err}");
                vec![]
            }
            Ok(Event::Block(block)) => {
                if block.level > dy.map.level() {
                    // TODO: do not fetch live blocks for any block, only for new level block
                    live_blocks = block.live_blocks.clone();
                    this_level.clear();
                    let delegates = srv.client.validators(block.level + 1)?;
                    dy.map.insert(block.level + 1, delegates);
                }
                this_level.insert(block.predecessor.clone());
                let reveal_ops = seed_nonce
                    .reveal_nonce(block.level)
                    .into_iter()
                    .map(|content| {
                        let mut bytes = block.predecessor.0.clone();
                        content.bin_write(&mut bytes).unwrap();
                        bytes.extend_from_slice(&[0; 64]);
                        hex::encode(bytes)
                    });
                for op_hex in reveal_ops {
                    match srv.client.inject_operation(&chain_id, op_hex, true) {
                        Ok(hash) => slog::info!(srv.log, " .  inject nonce_reveal: {hash}"),
                        Err(err) => slog::error!(srv.log, " .  {err}"),
                    }
                }
                let timestamp = tb::Timestamp {
                    unix_epoch: Duration::from_secs(block.timestamp),
                };
                let round = block.round;
                let pred_hash = tb::BlockHash(block.predecessor.0.as_slice().try_into().unwrap());
                slog::info!(
                    srv.log,
                    "Block: {}, pred: {}, {:?}",
                    block.hash,
                    block.predecessor,
                    block.fitness
                );
                if let Some(consensus) = block.operations.first() {
                    slog::debug!(
                        srv.log,
                        "Consensus operations: {}",
                        serde_json::to_string(consensus).unwrap()
                    );
                }
                let proposal = Box::new(tb::Block {
                    pred_hash,
                    level: block.level,
                    hash: tb::BlockHash(block.hash.0.as_slice().try_into().unwrap()),
                    time_header: tb::TimeHeader { round, timestamp },
                    payload: {
                        if !block.transition {
                            Some(tb::Payload {
                                hash: tb::PayloadHash(
                                    block.payload_hash.0.as_slice().try_into().unwrap(),
                                ),
                                payload_round: block.payload_round,
                                pre_cer: block.operations.first().and_then(|ops| {
                                    let v = ops
                                        .iter()
                                        .filter_map(|op| match op.kind()? {
                                            OperationKind::Preendorsement(v) => {
                                                Some((v, op.clone()))
                                            }
                                            _ => None,
                                        })
                                        .collect::<Vec<_>>();
                                    let (first, _) = v.first()?;
                                    let level = first.level;
                                    Some(tb::PreCertificate {
                                        payload_hash: {
                                            let c = first
                                                .block_payload_hash
                                                .0
                                                .as_slice()
                                                .try_into()
                                                .unwrap();
                                            tb::PayloadHash(c)
                                        },
                                        payload_round: first.round,
                                        votes: {
                                            v.into_iter()
                                                .filter_map(|(v, op)| {
                                                    dy.map.validator(level, v.slot, op)
                                                })
                                                .collect()
                                        },
                                    })
                                }),
                                cer: block.operations.first().and_then(|ops| {
                                    Some(tb::Certificate {
                                        votes: {
                                            ops.iter()
                                                .filter_map(|op| match op.kind()? {
                                                    OperationKind::Endorsement(v) => dy
                                                        .map
                                                        .validator(v.level, v.slot, op.clone()),
                                                    _ => None,
                                                })
                                                .collect()
                                        },
                                    })
                                }),
                                operations: block
                                    .operations
                                    .into_iter()
                                    .skip(1)
                                    .flatten()
                                    .collect(),
                            })
                        } else {
                            None
                        }
                    },
                });
                // TODO: investigate it
                let mut tries = 3;
                while tries > 0 {
                    tries -= 1;
                    if let Err(err) = srv.client.monitor_operations(WAIT_OPERATION_TIMEOUT) {
                        slog::error!(srv.log, " .  {}", err);
                    } else {
                        break;
                    }
                }
                let event = tb::Event::Proposal(proposal, now);
                let (mut actions, records) = state.handle(&dy, event);
                if let Some(ops) = ahead_ops.remove(&block.predecessor) {
                    process_ops(
                        ops,
                        &live_blocks,
                        &this_level,
                        &mut ahead_ops,
                        &dy,
                        &mut state,
                        &srv,
                        now,
                        &mut actions,
                    );
                }
                write_log(&srv.log, records);
                actions.into_iter().collect()
            }
            Ok(Event::Operations(ops)) => {
                let mut actions = vec![];
                process_ops(
                    ops,
                    &live_blocks,
                    &this_level,
                    &mut ahead_ops,
                    &dy,
                    &mut state,
                    &srv,
                    now,
                    &mut actions,
                );
                actions
            }
            Ok(Event::Tick) => {
                let event = tb::Event::Timeout;
                let (new_actions, records) = state.handle(&dy, event);
                write_log(&srv.log, records);
                new_actions.into_iter().collect()
            }
        };
        perform(
            &srv.client,
            &srv.log,
            &srv.timer,
            &srv.crypto,
            &mut seed_nonce,
            &dy.map,
            &chain_id,
            proof_of_work_threshold,
            actions,
        );
    }

    Ok(())
}

fn write_log(log: &Logger, records: impl IntoIterator<Item = tb::LogRecord>) {
    for record in records {
        match record.level() {
            tb::LogLevel::Info => slog::info!(log, "{record}"),
            tb::LogLevel::Warn => slog::warn!(log, "{record}"),
        }
    }
}

fn process_ops(
    ops: Vec<OperationSimple>,
    live_blocks: &Vec<BlockHash>,
    this_level: &BTreeSet<BlockHash>,
    ahead_ops: &mut BTreeMap<BlockHash, Vec<OperationSimple>>,
    dy: &tb::Config<tb::TimingLinearGrow, SlotsInfo>,
    state: &mut tb::Machine<ContractTz1Hash, OperationSimple, 200>,
    srv: &Services,
    now: tb::Timestamp,
    actions: &mut Vec<tb::Action<ContractTz1Hash, OperationSimple>>,
) {
    for op in ops {
        // the operation does not belong to
        let on_time = if this_level.contains(&op.branch) {
            true
        } else if live_blocks.contains(&op.branch) {
            false
        } else {
            slog::warn!(srv.log, " .  the op is ahead, or very outdated {op:?}");
            ahead_ops.entry(op.branch.clone()).or_default().push(op);
            continue;
        };
        match op.kind() {
            None => slog::error!(srv.log, " .  unclassified operation {op:?}"),
            Some(OperationKind::Preendorsement(content)) => {
                if !on_time {
                    slog::warn!(srv.log, " .  outdated preendorsement {op:?}");
                    continue;
                }
                if let Some(validator) = dy.map.validator(content.level, content.slot, op) {
                    let event = tb::Event::PreVoted(SlotsInfo::block_id(&content), validator, now);
                    let (new_actions, records) = state.handle(&dy, event);
                    write_log(&srv.log, records);
                    actions.extend(new_actions.into_iter());
                }
            }
            Some(OperationKind::Endorsement(content)) => {
                if !on_time {
                    slog::warn!(srv.log, " .  outdated endorsement {op:?}");
                    continue;
                }
                if let Some(validator) = dy.map.validator(content.level, content.slot, op) {
                    let event = tb::Event::Voted(SlotsInfo::block_id(&content), validator, now);
                    let (new_actions, records) = state.handle(&dy, event);
                    write_log(&srv.log, records);
                    actions.extend(new_actions.into_iter());
                }
            }
            Some(_) => {
                state.handle(&dy, tb::Event::Operation(op));
            }
        }
    }
}

fn perform(
    client: &RpcClient,
    log: &Logger,
    timer: &Timer,
    crypto: &CryptoService,
    seed_nonce: &mut SeedNonceService,
    slots_info: &SlotsInfo,
    chain_id: &ChainId,
    proof_of_work_threshold: u64,
    actions: impl IntoIterator<Item = tb::Action<ContractTz1Hash, OperationSimple>>,
) {
    for action in actions {
        match action {
            tb::Action::ScheduleTimeout(timestamp) => {
                timer.schedule(timestamp);
            }
            tb::Action::PreVote {
                pred_hash,
                block_id,
            } => {
                let this = crypto.public_key_hash();
                let slot = slots_info
                    .slots(&this, block_id.level)
                    .and_then(|v| v.first());
                let slot = match slot {
                    Some(s) => *s,
                    None => continue,
                };
                let preendorsement = InlinedPreendorsement {
                    branch: BlockHash(pred_hash.0.to_vec()),
                    operations: InlinedPreendorsementContents::Preendorsement(
                        InlinedPreendorsementVariant {
                            slot,
                            level: block_id.level,
                            round: block_id.round,
                            block_payload_hash: BlockPayloadHash(block_id.payload_hash.0.to_vec()),
                        },
                    ),
                    signature: Signature(vec![]),
                };
                let (data, _) = crypto.sign(0x12, &chain_id, &preendorsement).unwrap();
                match client.inject_operation(&chain_id, hex::encode(data), false) {
                    Ok(hash) => slog::info!(log, " .  inject preendorsement: {hash}"),
                    Err(err) => slog::error!(log, " .  {err}"),
                }
            }
            tb::Action::Vote {
                pred_hash,
                block_id,
            } => {
                let this = crypto.public_key_hash();
                let slot = slots_info
                    .slots(&this, block_id.level)
                    .and_then(|v| v.first());
                let slot = match slot {
                    Some(s) => *s,
                    None => continue,
                };
                let endorsement = InlinedEndorsement {
                    branch: BlockHash(pred_hash.0.to_vec()),
                    operations: InlinedEndorsementMempoolContents::Endorsement(
                        InlinedEndorsementMempoolContentsEndorsementVariant {
                            slot,
                            level: block_id.level,
                            round: block_id.round,
                            block_payload_hash: BlockPayloadHash(block_id.payload_hash.0.to_vec()),
                        },
                    ),
                    signature: Signature(vec![]),
                };
                let (data, _) = crypto.sign(0x13, &chain_id, &endorsement).unwrap();
                match client.inject_operation(&chain_id, hex::encode(data), false) {
                    Ok(hash) => slog::info!(log, " .  inject endorsement: {hash}"),
                    Err(err) => slog::error!(log, " .  {err}"),
                }
            }
            tb::Action::Propose(block, proposer, _repropose) => {
                // TODO: multiple bakers
                let _ = proposer;
                let payload = match block.payload {
                    Some(v) => v,
                    None => return,
                };

                let predecessor_hash = BlockHash(block.pred_hash.0.to_vec());
                let endorsements = payload
                    .cer
                    .map(|q| q.votes.ids.into_values())
                    .into_iter()
                    .flatten();
                let preendorsements = payload
                    .pre_cer
                    .map(|q| q.votes.ids.into_values())
                    .into_iter()
                    .flatten();
                let mut operations = [
                    endorsements.chain(preendorsements).collect::<Vec<_>>(),
                    vec![],
                    vec![],
                    vec![],
                ];
                for op in payload.operations {
                    match op.kind() {
                        None => {
                            slog::warn!(log, " .  unclassified operation {op:?}");
                        }
                        Some(OperationKind::Endorsement(_) | OperationKind::Preendorsement(_)) => {
                            slog::warn!(log, " .  unexpected consensus operation {op:?}");
                        }
                        Some(OperationKind::Votes) => operations[1].push(op),
                        Some(OperationKind::Anonymous) => {
                            let mut op = op;
                            if op.signature.is_none() {
                                op.signature = Some(Signature(vec![0; 64]));
                            }
                            operations[2].push(op)
                        }
                        Some(OperationKind::Managers) => operations[3].push(op),
                    }
                }

                let payload_round = payload.payload_round;
                let payload_hash = if payload.hash.0 == [0; 32] {
                    let hashes = operations[1..]
                        .as_ref()
                        .iter()
                        .flatten()
                        .filter_map(|op| op.hash.as_ref().cloned())
                        .collect::<Vec<_>>();
                    let operation_list_hash = OperationListHash::calculate(&hashes).unwrap();
                    BlockPayloadHash::calculate(
                        &predecessor_hash,
                        payload_round as u32,
                        &operation_list_hash,
                    )
                    .unwrap()
                } else {
                    BlockPayloadHash(payload.hash.0.to_vec())
                };
                let seed_nonce_hash = seed_nonce.gen_nonce(block.level).unwrap();
                let mut protocol_header = ProtocolBlockHeader {
                    payload_hash,
                    payload_round,
                    seed_nonce_hash,
                    proof_of_work_nonce: SizedBytes(
                        hex::decode("7985fafe1fb70300").unwrap().try_into().unwrap(),
                    ),
                    liquidity_baking_escape_vote: false,
                    signature: Signature(vec![]),
                };
                let (_, signature) = crypto.sign(0x11, &chain_id, &protocol_header).unwrap();
                protocol_header.signature = signature;
                let timestamp = block.time_header.timestamp.unix_epoch.as_secs() as i64;

                let (mut header, operations) = match client.preapply_block(
                    protocol_header,
                    predecessor_hash.clone(),
                    timestamp,
                    operations,
                ) {
                    Ok(v) => v,
                    Err(err) => {
                        slog::error!(log, " .  {err}");
                        continue;
                    }
                };

                header.signature.0 = vec![0x00; 64];
                let p = guess_proof_of_work(&header, proof_of_work_threshold);
                header.proof_of_work_nonce = SizedBytes(p);
                slog::info!(log, "{:?}", header);
                header.signature.0.clear();
                let (data, _) = crypto.sign(0x11, &chain_id, &header).unwrap();

                let valid_operations = operations
                    .iter()
                    .filter_map(|v| {
                        let applied = v.as_object()?.get("applied")?.clone();
                        serde_json::from_value(applied).ok()
                    })
                    .collect();

                match client.inject_block(hex::encode(data), valid_operations) {
                    Ok(hash) => slog::info!(
                        log,
                        " .  inject block: {}:{}, {hash}",
                        header.level,
                        block.time_header.round
                    ),
                    Err(err) => {
                        slog::error!(log, " .  {err}");
                        slog::error!(log, " .  {}", serde_json::to_string(&operations).unwrap());
                    }
                }
            }
        }
    }
}
