// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
    fmt, mem,
    sync::Arc,
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crypto::hash::{
    BlockHash, BlockPayloadHash, ChainId, ContractTz1Hash, OperationListHash, Signature,
};
use tenderbake as tb;
use tezos_encoding::types::SizedBytes;
use tezos_messages::protocol::proto_012::operation::{
    EndorsementOperation, InlinedEndorsement, InlinedEndorsementMempoolContents,
    InlinedEndorsementMempoolContentsEndorsementVariant, InlinedPreendorsement,
    InlinedPreendorsementContents, InlinedPreendorsementVariant,
};

use crate::services::{
    client::{Constants, ProtocolBlockHeader, RpcError},
    event::{Block, OperationKind, OperationSimple},
    ActionInner, EventWithTime,
};

use super::{
    actions::*,
    cycle_nonce::CycleNonce,
    request::{Request, RequestState},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct SlotsInfo {
    pub committee_size: u32,
    pub ours: Vec<ContractTz1Hash>,
    pub level: i32,
    pub delegates: BTreeMap<i32, BTreeMap<ContractTz1Hash, Vec<u16>>>,
}

pub enum Gathering {
    // for some `level: i32` we request a collection of public key hash
    // and corresponding slots
    GetSlots(Request<i32, BTreeMap<ContractTz1Hash, Vec<u16>>, Arc<RpcError>>),
    // for some `BlockHash` we request its operations
    GetOperations(Request<BlockHash, Vec<Vec<OperationSimple>>, Arc<RpcError>>),
    // for some `BlockHash` we request a list of live blocks
    GetLiveBlocks(Request<BlockHash, Vec<BlockHash>, Arc<RpcError>>),
}

impl fmt::Display for Gathering {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Gathering::GetSlots(r) => write!(f, "slots {r}"),
            Gathering::GetOperations(r) => write!(f, "operations {r}"),
            Gathering::GetLiveBlocks(r) => write!(f, "live blocks {r}"),
        }
    }
}

pub enum BakerState {
    Idle(Initialized),
    Gathering {
        state: Initialized,
        gathering: Gathering,
        current_block: Block,
    },
    HaveBlock {
        state: Initialized,
        current_block: Block,
    },
    Invalid {
        state: Initialized,
        error: Arc<RpcError>,
    },
}

impl fmt::Display for BakerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BakerState::Idle(_) => write!(f, "idle"),
            BakerState::Gathering { gathering, .. } => write!(f, "gathering {gathering}"),
            BakerState::HaveBlock { current_block, .. } => {
                write!(
                    f,
                    "have block {}:{}",
                    current_block.level, current_block.round
                )
            }
            BakerState::Invalid { error, .. } => write!(f, "invalid {error}"),
        }
    }
}

pub struct Initialized {
    pub chain_id: ChainId,
    pub proof_of_work_threshold: u64,
    pub this: ContractTz1Hash,
    // cycle state
    pub nonces: CycleNonce,
    // operations which came ahead of the block stored heres
    pub ahead_ops: BTreeMap<BlockHash, Vec<OperationSimple>>,
    // live blocks
    pub live_blocks: Vec<BlockHash>,
    // blocks at this level and their predecessors
    pub this_level: BTreeSet<BlockHash>,
    // operations in this proposal
    pub operations: Vec<Vec<OperationSimple>>,
    // tenderbake machine
    pub tb_config: tb::Config<tb::TimingLinearGrow, SlotsInfo>,
    pub tb_state: tb::Machine<ContractTz1Hash, OperationSimple, 200>,

    pub actions: Vec<ActionInner>,
}

pub struct BakerStateEjectable(pub Option<BakerState>);

impl AsRef<Option<BakerState>> for BakerStateEjectable {
    fn as_ref(&self) -> &Option<BakerState> {
        &self.0
    }
}

impl AsMut<Option<BakerState>> for BakerStateEjectable {
    fn as_mut(&mut self) -> &mut Option<BakerState> {
        &mut self.0
    }
}

impl AsRef<Initialized> for BakerState {
    fn as_ref(&self) -> &Initialized {
        match self {
            BakerState::Idle(state) => state,
            BakerState::Gathering { state, .. } => state,
            BakerState::HaveBlock { state, .. } => state,
            BakerState::Invalid { state, .. } => state,
        }
    }
}

impl AsMut<Initialized> for BakerState {
    fn as_mut(&mut self) -> &mut Initialized {
        match self {
            BakerState::Idle(state) => state,
            BakerState::Gathering { state, .. } => state,
            BakerState::HaveBlock { state, .. } => state,
            BakerState::Invalid { state, .. } => state,
        }
    }
}

impl BakerState {
    pub fn new(chain_id: ChainId, constants: Constants, this: ContractTz1Hash) -> Self {
        let timing = tb::TimingLinearGrow {
            minimal_block_delay: constants.minimal_block_delay,
            delay_increment_per_round: constants.delay_increment_per_round,
        };

        let ours = vec![this.clone()];
        let tb_config = tb::Config {
            timing,
            map: SlotsInfo {
                committee_size: constants.consensus_committee_size,
                ours,
                level: 0,
                delegates: BTreeMap::new(),
            },
            quorum: 2 * (constants.consensus_committee_size / 3) + 1,
        };

        BakerState::Idle(Initialized {
            chain_id,
            proof_of_work_threshold: constants.proof_of_work_threshold,
            this,
            nonces: CycleNonce {
                blocks_per_commitment: constants.blocks_per_commitment,
                blocks_per_cycle: constants.blocks_per_cycle,
                nonce_length: constants.nonce_length,
                cycle: 0,
                previous: BTreeMap::new(),
                this: BTreeMap::new(),
            },
            ahead_ops: BTreeMap::new(),
            live_blocks: Vec::new(),
            this_level: BTreeSet::new(),
            operations: Vec::new(),
            tb_config,
            tb_state: tb::Machine::<ContractTz1Hash, OperationSimple, 200>::default(),
            actions: vec![],
        })
    }

    pub fn handle_event(mut self, event: EventWithTime) -> Self {
        // those are already executed
        self.as_mut().actions.clear();

        self.handle_event_inner(event)
    }

    #[rustfmt::skip]
    fn handle_event_inner(mut self, event: EventWithTime) -> Self {
        let EventWithTime { event, now } = event;

        let description = self.to_string();
        if !matches!(&event, BakerAction::OperationsEvent(_)) {
            self.as_mut().actions.push(ActionInner::LogInfo {
                with_prefix: false,
                description,
            });
        }

        match event {
            BakerAction::RpcError(RpcErrorAction { error }) => {
                self.as_mut().actions.push(ActionInner::LogError(format!("{error}")));
                let state = self.into_inner();
                BakerState::Invalid { state, error }
            }
            BakerAction::IdleEvent(IdleEventAction {}) => {
                match self {
                    BakerState::Gathering {
                        state,
                        gathering: Gathering::GetSlots(Request {
                            id: _,
                            state: RequestState::Error(error),
                        }),
                        current_block: _,
                    } => BakerState::Invalid { state, error },
                    BakerState::Gathering {
                        state,
                        gathering: Gathering::GetOperations(Request {
                            id: _,
                            state: RequestState::Error(error),
                        }),
                        current_block: _,
                    } => BakerState::Invalid { state, error },
                    BakerState::Gathering {
                        state,
                        gathering: Gathering::GetLiveBlocks(Request {
                            id: _,
                            state: RequestState::Error(error),
                        }),
                        current_block: _,
                    } => BakerState::Invalid { state, error },
                    BakerState::Gathering {
                        mut state,
                        gathering: Gathering::GetSlots(Request {
                            id: level,
                            state: RequestState::Success(delegates),
                        }),
                        current_block,
                    } => {
                        state.tb_config.map.level = level - 1;
                        state.tb_config.map.delegates.insert(level, delegates);
                        state.actions.push(ActionInner::GetOperationsForBlock { block_hash: current_block.hash.clone() });
                        BakerState::Gathering {
                            state,
                            gathering: Gathering::GetOperations(Request::new(current_block.hash.clone())),
                            current_block,
                        }
                    },
                    BakerState::Gathering {
                        mut state,
                        gathering: Gathering::GetOperations(Request {
                            id: _,
                            state: RequestState::Success(operations),
                        }),
                        current_block,
                    } => {
                        state.operations = operations;
                        if state.tb_state.elected_block().is_none() {
                            // if we have no elected block, ask a new live blocks list
                            state.actions.push(ActionInner::GetLiveBlocks { block_hash: current_block.hash.clone() });
                            BakerState::Gathering {
                                state,
                                gathering: Gathering::GetLiveBlocks(Request::new(current_block.hash.clone())),
                                current_block,
                            }
                        } else {
                            state.actions.push(ActionInner::Idle);
                            BakerState::HaveBlock { state, current_block }
                        }
                    },
                    BakerState::Gathering {
                        mut state,
                        gathering: Gathering::GetLiveBlocks(Request {
                            id: _,
                            state: RequestState::Success(live_blocks),
                        }),
                        current_block,
                    } => {
                        state.live_blocks = live_blocks;
                        state.actions.push(ActionInner::Idle);
                        BakerState::HaveBlock { state, current_block }
                    },
                    BakerState::HaveBlock { mut state, current_block } => {
                        state.actions.push(ActionInner::MonitorOperations);
                        let operations = mem::take(&mut state.operations);
                        let proposal = Box::new(proposal(&current_block, operations, &state.tb_config));
                        let (tb_actions, records) = state.tb_state.handle(&state.tb_config, tb::Event::Proposal(proposal, now));
                        state.actions.extend(records.into_iter().map(ActionInner::LogTb));
                        let description = format!("hash: {}, predecessor: {}", current_block.hash, current_block.predecessor);
                        state.actions.push(ActionInner::LogInfo {
                            with_prefix: true,
                            description,
                        });
                        state.handle_tb_actions(tb_actions);
                        if let Some(operations) = state.ahead_ops.remove(&current_block.predecessor) {
                            BakerState::Idle(state).handle_event_inner(EventWithTime {
                                event: BakerAction::OperationsEvent(OperationsEventAction { operations }),
                                now,
                            })
                        } else {
                            BakerState::Idle(state)
                        }
                    },
                    s => s,
                }
            }
            BakerAction::ProposalEvent(ProposalEventAction { block }) => {
                // dbg!(format!("handle in state machine: {}:{}", block.level, block.round));
                let state = self.as_mut();
                let gathering = if block.level > state.tb_config.map.level {
                    // a new level
                    state.this_level.clear();
                    state.actions.push(ActionInner::GetSlots {
                        level: block.level + 1,
                    });
                    Gathering::GetSlots(Request::new(block.level + 1))
                } else {
                    // the same level
                    state.actions.push(ActionInner::GetOperationsForBlock {
                        block_hash: block.hash.clone(),
                    });
                    Gathering::GetOperations(Request::new(block.hash.clone()))
                };
                state.this_level.insert(block.hash.clone());
                state.this_level.insert(block.predecessor.clone());

                let chain_id = state.chain_id.clone();
                let nonces = state.nonces.reveal_nonce(block.level);
                let branch = block.predecessor.clone();
                let nonces = nonces.map(|(level, nonce)| ActionInner::RevealNonce {
                    chain_id: chain_id.clone(),
                    branch: branch.clone(),
                    level,
                    nonce,
                });
                state.actions.extend(nonces);

                BakerState::Gathering {
                    state: self.into_inner(),
                    current_block: block,
                    gathering,
                }
            }
            BakerAction::SlotsEvent(SlotsEventAction { level, delegates }) => match self {
                BakerState::Gathering {
                    mut state,
                    current_block,
                    gathering: Gathering::GetSlots(r),
                } if r.is_pending() && level == r.id => {
                    state.actions.push(ActionInner::Idle);
                    BakerState::Gathering {
                        state,
                        gathering: Gathering::GetSlots(r.done_ok(delegates)),
                        current_block,
                    }
                }
                s => s,
            },
            BakerAction::OperationsForBlockEvent(OperationsForBlockEventAction { block_hash, operations }) => match self {
                BakerState::Gathering {
                    mut state,
                    gathering: Gathering::GetOperations(r),
                    current_block,
                } if r.is_pending() && block_hash == r.id => {
                    state.actions.push(ActionInner::Idle);
                    BakerState::Gathering {
                        state,
                        gathering: Gathering::GetOperations(r.done_ok(operations)),
                        current_block,
                    }
                }
                s => s,
            },
            BakerAction::LiveBlocksEvent(LiveBlocksEventAction { block_hash, live_blocks }) => match self {
                BakerState::Gathering {
                    mut state,
                    current_block,
                    gathering: Gathering::GetLiveBlocks(r),
                } if r.is_pending() && block_hash == r.id => {
                    state.actions.push(ActionInner::Idle);
                    BakerState::Gathering {
                        state,
                        gathering: Gathering::GetLiveBlocks(r.done_ok(live_blocks)),
                        current_block,
                    }
                }
                s => s,
            },
            BakerAction::OperationsEvent(OperationsEventAction { operations }) => {
                let state = self.as_mut();
                for op in operations {
                    match op.kind() {
                        None => {
                            state.actions.push(ActionInner::LogError(format!("unclassified operation {op:?}")))
                        }
                        Some(OperationKind::Preendorsement(content)) => {
                            if !state.this_level.contains(&op.branch) {
                                state.actions.push(ActionInner::LogWarning(format!("the op is ahead, or very outdated {op:?}")));
                                state.ahead_ops.entry(op.branch.clone()).or_default().push(op);
                                continue;
                            };
                            if let Some(validator) =
                                state
                                    .tb_config
                                    .map
                                    .validator(content.level, content.slot, op)
                            {
                                let event = tb::Event::PreVoted(block_id(&content), validator, now);
                                let (tb_actions, records) =
                                    state.tb_state.handle(&state.tb_config, event);
                                state.actions.extend(records.into_iter().map(ActionInner::LogTb));
                                state.handle_tb_actions(tb_actions);
                            }
                        }
                        Some(OperationKind::Endorsement(content)) => {
                            if let Some(validator) =
                                state
                                    .tb_config
                                    .map
                                    .validator(content.level, content.slot, op)
                            {
                                let event = tb::Event::Voted(block_id(&content), validator, now);
                                let (tb_actions, records) =
                                    state.tb_state.handle(&state.tb_config, event);
                                state.actions.extend(records.into_iter().map(ActionInner::LogTb));
                                state.handle_tb_actions(tb_actions);
                            }
                        }
                        Some(_) => {
                            // the operation does not belong to live_blocks
                            if !state.live_blocks.contains(&op.branch) {
                                state.actions.push(ActionInner::LogWarning(format!("the op is outdated {op:?}")));
                                state.ahead_ops.entry(op.branch.clone()).or_default().push(op);
                                continue;
                            };
                            state
                                .tb_state
                                .handle(&state.tb_config, tb::Event::Operation(op));
                        }
                    }
                }
                self
            }
            BakerAction::TickEvent(TickEventAction {}) => {
                let state = self.as_mut();
                let (tb_actions, records) =
                    state.tb_state.handle(&state.tb_config, tb::Event::Timeout);
                state.actions.extend(records.into_iter().map(ActionInner::LogTb));
                state.handle_tb_actions(tb_actions);
                self
            }
            _ => self,
        }
    }

    fn into_inner(self) -> Initialized {
        match self {
            BakerState::Idle(state) => state,
            BakerState::Gathering { state, .. } => state,
            BakerState::HaveBlock { state, .. } => state,
            BakerState::Invalid { state, .. } => state,
        }
    }
}

impl Initialized {
    fn handle_tb_actions(&mut self, tb_actions: Vec<tb::Action<ContractTz1Hash, OperationSimple>>) {
        for tb_action in tb_actions {
            match tb_action {
                tb::Action::ScheduleTimeout(t) => {
                    self.actions.push(ActionInner::ScheduleTimeout(t));
                }
                tb::Action::Propose(block, _, _) => {
                    self.propose(*block);
                }
                tb::Action::PreVote {
                    pred_hash,
                    block_id,
                } => {
                    self.pre_vote(pred_hash, block_id);
                }
                tb::Action::Vote {
                    pred_hash,
                    block_id,
                } => {
                    self.vote(pred_hash, block_id);
                }
            }
        }
    }

    fn pre_vote(&mut self, pred_hash: tb::BlockHash, block_id: tb::BlockId) {
        let slot = self
            .tb_config
            .map
            .delegates
            .get(&block_id.level)
            .and_then(|v| v.get(&self.this))
            .and_then(|v| v.first());
        let slot = match slot {
            Some(s) => *s,
            None => return,
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
        self.actions
            .push(ActionInner::PreVote(self.chain_id.clone(), preendorsement));
    }

    fn vote(&mut self, pred_hash: tb::BlockHash, block_id: tb::BlockId) {
        let slot = self
            .tb_config
            .map
            .delegates
            .get(&block_id.level)
            .and_then(|v| v.get(&self.this))
            .and_then(|v| v.first());
        let slot = match slot {
            Some(s) => *s,
            None => return,
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
        self.actions
            .push(ActionInner::Vote(self.chain_id.clone(), endorsement));
    }

    fn propose(&mut self, block: tb::Block<ContractTz1Hash, OperationSimple>) {
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
        let mut hashes = BTreeSet::new();
        for op in payload.operations {
            op.hash.as_ref().unwrap();
            if let Some(hash) = &op.hash {
                if !hashes.insert(hash.clone()) {
                    continue;
                }
            }
            match op.kind() {
                None => {
                    let s = format!("unclassified operation {op:?}");
                    self.actions.push(ActionInner::LogWarning(s));
                }
                Some(OperationKind::Endorsement(_) | OperationKind::Preendorsement(_)) => {
                    let s = format!("unexpected consensus operation {op:?}");
                    self.actions.push(ActionInner::LogWarning(s));
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
        let payload_hash = if payload.hash != tb::PayloadHash([0; 32]) {
            BlockPayloadHash(payload.hash.0.to_vec())
        } else {
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
        };
        let seed_nonce_hash = self.nonces.gen_nonce(block.level);
        let protocol_header = ProtocolBlockHeader {
            payload_hash,
            payload_round,
            seed_nonce_hash,
            proof_of_work_nonce: SizedBytes(0x7985fafe1fb70300u64.to_be_bytes()),
            liquidity_baking_escape_vote: false,
            signature: Signature(vec![]),
        };
        let timestamp = block.time_header.timestamp.unix_epoch.as_secs() as i64;

        self.actions.push(ActionInner::Propose {
            chain_id: self.chain_id.clone(),
            proof_of_work_threshold: self.proof_of_work_threshold,
            protocol_header,
            predecessor_hash,
            operations,
            timestamp,
            round: block.time_header.round,
        })
    }
}

impl tb::ProposerMap for SlotsInfo {
    type Id = ContractTz1Hash;

    fn proposer(&self, level: i32, round: i32) -> Option<(i32, Self::Id)> {
        self.ours
            .iter()
            .filter_map(|our| {
                self.delegates
                    .get(&level)?
                    .get(our)
                    .into_iter()
                    .flatten()
                    .skip_while(|c| **c < (round as u32 % self.committee_size) as u16)
                    .next()
                    .map(|r| (*r as i32, our.clone()))
            })
            .min_by(|(a, _), (b, _)| a.cmp(b))
    }
}

impl SlotsInfo {
    fn validator(
        &self,
        level: i32,
        slot: u16,
        operation: OperationSimple,
    ) -> Option<tb::Validator<ContractTz1Hash, OperationSimple>> {
        let i = self.delegates.get(&level)?;
        let (id, s) = i.iter().find(|&(_, v)| v.first() == Some(&slot))?;
        Some(tb::Validator {
            id: id.clone(),
            power: s.len() as u32,
            operation,
        })
    }
}

fn block_id(content: &EndorsementOperation) -> tb::BlockId {
    tb::BlockId {
        level: content.level,
        round: content.round,
        payload_hash: {
            let c = content
                .block_payload_hash
                .0
                .as_slice()
                .try_into()
                .expect("payload hash is 32 bytes");
            tb::PayloadHash(c)
        },
    }
}

fn proposal(
    block: &Block,
    operations: Vec<Vec<OperationSimple>>,
    tb_config: &tb::Config<tb::TimingLinearGrow, SlotsInfo>,
) -> tb::Block<ContractTz1Hash, OperationSimple> {
    tb::Block {
        pred_hash: tb::BlockHash(block.predecessor.0.as_slice().try_into().unwrap()),
        level: block.level,
        hash: tb::BlockHash(block.hash.0.as_slice().try_into().unwrap()),
        time_header: tb::TimeHeader {
            round: block.round,
            timestamp: tb::Timestamp {
                unix_epoch: Duration::from_secs(block.timestamp),
            },
        },
        payload: {
            if !block.transition {
                Some(tb::Payload {
                    hash: tb::PayloadHash(block.payload_hash.0.as_slice().try_into().unwrap()),
                    payload_round: block.payload_round,
                    pre_cer: operations.first().and_then(|ops| {
                        let v = ops
                            .iter()
                            .filter_map(|op| match op.kind()? {
                                OperationKind::Preendorsement(v) => Some((v, op.clone())),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        let (first, _) = v.first()?;
                        let level = first.level;
                        Some(tb::PreCertificate {
                            payload_hash: {
                                let c = first.block_payload_hash.0.as_slice().try_into().unwrap();
                                tb::PayloadHash(c)
                            },
                            payload_round: first.round,
                            votes: {
                                v.into_iter()
                                    .filter_map(|(v, op)| {
                                        tb_config.map.validator(level, v.slot, op)
                                    })
                                    .collect()
                            },
                        })
                    }),
                    cer: operations.first().and_then(|ops| {
                        Some(tb::Certificate {
                            votes: {
                                ops.iter()
                                    .filter_map(|op| match op.kind()? {
                                        OperationKind::Endorsement(v) => {
                                            tb_config.map.validator(v.level, v.slot, op.clone())
                                        }
                                        _ => None,
                                    })
                                    .collect()
                            },
                        })
                    }),
                    operations: operations.into_iter().skip(1).flatten().collect(),
                })
            } else {
                None
            }
        },
    }
}
