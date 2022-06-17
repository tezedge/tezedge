// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, mem,
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crypto::hash::{ChainId, Signature};
use tezos_messages::protocol::proto_012::operation::{
    InlinedEndorsement, InlinedEndorsementMempoolContents,
    InlinedEndorsementMempoolContentsEndorsementVariant, InlinedPreendorsement,
    InlinedPreendorsementContents, InlinedPreendorsementVariant,
};

use crate::services::{
    client::{Constants, LiquidityBakingToggleVote, Protocol},
    event::{Block, Slots},
    EventWithTime,
};
use crate::tenderbake_new::{self as tb, hash, NoValue};

use super::{
    actions::*,
    cycle_nonce::CycleNonce,
    request::{Request, RequestState},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct SlotsInfo {
    pub committee_size: u32,
    pub ours: Vec<hash::ContractTz1Hash>,
    pub level: i32,
    pub delegates: BTreeMap<i32, BTreeMap<hash::ContractTz1Hash, Slots>>,
}

#[derive(Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
pub enum Gathering {
    GetCornerSlots(Request<i32, BTreeMap<hash::ContractTz1Hash, Slots>, String>),
    // for some `level: i32` we request a collection of public key hash
    // and corresponding slots
    GetSlots(Request<i32, BTreeMap<hash::ContractTz1Hash, Slots>, String>),
    // for some `BlockHash` we request its operations
    GetOperations(Request<hash::BlockHash, Vec<Vec<tb::OperationSimple>>, String>),
    // for some `BlockHash` we request a list of live blocks
    GetLiveBlocks(Request<hash::BlockHash, Vec<hash::BlockHash>, String>),
}

impl fmt::Display for Gathering {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Gathering::GetCornerSlots(r) => write!(f, "corner slots {r}"),
            Gathering::GetSlots(r) => write!(f, "slots {r}"),
            Gathering::GetOperations(r) => write!(f, "operations {r}"),
            Gathering::GetLiveBlocks(r) => write!(f, "live blocks {r}"),
        }
    }
}

#[derive(Serialize, Deserialize)]
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
        error: String,
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

#[derive(Serialize, Deserialize)]
pub struct Initialized {
    #[serde(default)]
    pub protocol: Protocol,
    #[serde(default)]
    pub liquidity_baking_toggle_vote: LiquidityBakingToggleVote,
    pub chain_id: ChainId,
    pub proof_of_work_threshold: u64,
    pub this: hash::ContractTz1Hash,
    // cycle state
    pub nonces: CycleNonce,
    // live blocks
    pub live_blocks: (hash::BlockHash, Vec<hash::BlockHash>),
    // operations in this proposal
    pub operations: Vec<Vec<tb::OperationSimple>>,
    #[serde(default)]
    pub new_operations: Vec<tb::OperationSimple>,
    // tenderbake machine
    pub tb_config: tb::Config<tb::TimingLinearGrow, SlotsInfo>,
    pub tb_state: tb::Machine,

    pub actions: Vec<BakerAction>,
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
    pub fn new(
        chain_id: ChainId,
        constants: Constants,
        this: hash::ContractTz1Hash,
        protocol: Protocol,
        liquidity_baking_toggle_vote: LiquidityBakingToggleVote,
    ) -> Self {
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
            protocol,
            liquidity_baking_toggle_vote,
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
            live_blocks: (hash::BlockHash::no_value(), Vec::new()),
            operations: Vec::new(),
            new_operations: Vec::new(),
            tb_config,
            tb_state: tb::Machine::default(),
            actions: vec![],
        })
    }

    pub fn handle_event(mut self, event: EventWithTime) -> Self {
        // those are already executed
        self.as_mut().actions.clear();
        if event.action.is_event() {
            self.handle_event_inner(event)
        } else {
            self
        }
    }

    #[rustfmt::skip]
    fn handle_event_inner(mut self, event: EventWithTime) -> Self {
        let EventWithTime { action, now } = event;

        let description = self.to_string();
        if !matches!(&action, BakerAction::OperationsEvent(_)) {
            self.as_mut().actions.push(BakerAction::LogInfo(LogInfoAction {
                with_prefix: false,
                description,
            }));
        }

        match action {
            BakerAction::RpcError(RpcErrorAction { error }) => {
                self.as_mut().actions.push(BakerAction::LogError(LogErrorAction { description: error.clone() }));
                let state = self.into_inner();
                BakerState::Invalid { state, error }
            }
            BakerAction::IdleEvent(IdleEventAction {}) => {
                match self {
                    BakerState::Gathering {
                        state,
                        gathering: Gathering::GetCornerSlots(Request {
                            id: _,
                            state: RequestState::Error(error),
                        }),
                        current_block: _,
                    } => BakerState::Invalid { state, error },
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
                        gathering: Gathering::GetCornerSlots(Request {
                            id: level,
                            state: RequestState::Success(delegates),
                        }),
                        current_block,
                    } => {
                        state.tb_config.map.delegates.insert(level, delegates);
                        state.actions.push(BakerAction::GetSlots(GetSlotsAction {
                            level: level + 1,
                        }));
                        BakerState::Gathering {
                            state,
                            gathering: Gathering::GetSlots(Request::new(level + 1)),
                            current_block,
                        }
                    }
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
                        // keep only prev level, this level and next level 
                        // (level - 2, level - 1, level)
                        state.tb_config.map.delegates.remove(&(level - 3));
                        state.actions.push(BakerAction::GetOperationsForBlock(GetOperationsForBlockAction { block_hash: current_block.hash.clone() }));
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
                        if state.tb_state.elected_block().is_none() || Some(current_block.level) > state.tb_state.level() {
                            // if we have no elected block, or it is on previous level,
                            // ask a new live blocks list
                            state.actions.push(BakerAction::GetLiveBlocks(GetLiveBlocksAction { block_hash: current_block.hash.clone() }));
                            BakerState::Gathering {
                                state,
                                gathering: Gathering::GetLiveBlocks(Request::new(current_block.hash.clone())),
                                current_block,
                            }
                        } else {
                            state.actions.push(BakerAction::Idle(IdleAction {}));
                            BakerState::HaveBlock { state, current_block }
                        }
                    },
                    BakerState::Gathering {
                        mut state,
                        gathering: Gathering::GetLiveBlocks(Request {
                            id,
                            state: RequestState::Success(live_blocks),
                        }),
                        current_block,
                    } => {
                        state.live_blocks = (id, live_blocks);
                        state.actions.push(BakerAction::Idle(IdleAction {}));
                        BakerState::HaveBlock { state, current_block }
                    },
                    BakerState::HaveBlock { mut state, current_block } => {
                        state.actions.push(BakerAction::MonitorOperations(MonitorOperationsAction {}));
                        let operations = mem::take(&mut state.operations);
                        let proposal = proposal(&current_block, operations, &state.tb_config);
                        let mut tb_actions = vec![];
                        state.tb_state.handle(&state.tb_config, tb::Event::Proposal(proposal, now), &mut tb_actions);
                        let description = format!("hash: {}, predecessor: {}", current_block.hash, current_block.predecessor);
                        state.actions.push(BakerAction::LogInfo(LogInfoAction {
                            with_prefix: true,
                            description,
                        }));
                        state.handle_tb_actions(tb_actions);
                        BakerState::Idle(state)
                    },
                    s => s,
                }
            }
            BakerAction::ProposalEvent(ProposalEventAction { block }) => {
                // dbg!(format!("handle in state machine: {}:{}", block.level, block.round));
                let state = self.as_mut();
                if block.level < state.tb_config.map.level {
                    let description = "old_block".to_string();
                    state.actions.push(BakerAction::LogWarning(LogWarningAction { description }));
                }
                let gathering = if block.level > state.tb_config.map.level {
                    // a new level
                    if !state.tb_config.map.delegates.contains_key(&block.level) {
                        state.actions.push(BakerAction::GetSlots(GetSlotsAction {
                            level: block.level,
                        }));
                        Gathering::GetCornerSlots(Request::new(block.level))
                    } else {
                        state.actions.push(BakerAction::GetSlots(GetSlotsAction {
                            level: block.level + 1,
                        }));
                        Gathering::GetSlots(Request::new(block.level + 1))
                    }
                } else {
                    // the same level
                    state.actions.push(BakerAction::GetOperationsForBlock(GetOperationsForBlockAction {
                        block_hash: block.hash.clone(),
                    }));
                    Gathering::GetOperations(Request::new(block.hash.clone()))
                };

                // already final block is block.level - 2
                let final_block_level = block.level - 2;
                let nonces = state.nonces.reveal_nonce(final_block_level);
                let branch = block.predecessor.clone();
                let nonces = nonces.map(|(level, nonce)| BakerAction::RevealNonce(RevealNonceAction {
                    branch: branch.clone(),
                    level,
                    nonce,
                }));
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
                    gathering: Gathering::GetCornerSlots(r),
                    current_block,
                } if r.is_pending() && level == r.id => {
                    state.actions.push(BakerAction::Idle(IdleAction {}));
                    BakerState::Gathering {
                        state,
                        gathering: Gathering::GetCornerSlots(r.done_ok(delegates)),
                        current_block,
                    }
                }
                BakerState::Gathering {
                    mut state,
                    current_block,
                    gathering: Gathering::GetSlots(r),
                } if r.is_pending() && level == r.id => {
                    state.actions.push(BakerAction::Idle(IdleAction {}));
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
                    state.actions.push(BakerAction::Idle(IdleAction {}));
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
                    state.actions.push(BakerAction::Idle(IdleAction {}));
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
                    match op {
                        tb::OperationSimple::Preendorsement(op) => {
                            if let Some((id, power)) =
                                state
                                    .tb_config
                                    .map
                                    .validator(op.contents.level, op.contents.slot)
                            {
                                let vote = tb::Vote {
                                    id,
                                    power,
                                    op: op.map(tb::ConsensusBody::block_id),
                                };
                                let event = tb::Event::Preendorsed { vote };
                                let mut tb_actions = vec![];
                                state.tb_state.handle(&state.tb_config, event, &mut tb_actions);
                                state.handle_tb_actions(tb_actions);
                            }
                        }
                        tb::OperationSimple::Endorsement(op) => {
                            if let Some((id, power)) =
                                state
                                    .tb_config
                                    .map
                                    .validator(op.contents.level, op.contents.slot)
                            {
                                let vote = tb::Vote {
                                    id,
                                    power,
                                    op: op.map(tb::ConsensusBody::block_id),
                                };
                                let event = tb::Event::Endorsed { vote, now };
                                let mut tb_actions = vec![];
                                state.tb_state.handle(&state.tb_config, event, &mut tb_actions);
                                state.handle_tb_actions(tb_actions);
                            }
                        }
                        #[cfg(not(feature = "testing-mock"))]
                        _ => {
                            if state.live_blocks.0 != state.tb_state.hash().as_ref().unwrap().clone() {
                                state.new_operations.push(op);
                                continue;
                            }
                            for op in mem::take(&mut state.new_operations).into_iter().chain(std::iter::once(op)) {
                                // the operation does not belong to live_blocks
                                if !state.live_blocks.1.contains(&op.branch()) {
                                    let description = format!("the op is outdated {op:?}");
                                    state.actions.push(BakerAction::LogWarning(LogWarningAction { description }));
                                    // state.ahead_ops.entry(op.branch.clone()).or_default().push(op);
                                    continue;
                                };
                                state
                                    .tb_state
                                    .handle(&state.tb_config, tb::Event::Operation(op), &mut vec![]);
                            }
                        }
                    }
                }
                self
            }
            BakerAction::TickEvent(TickEventAction { scheduled_at_level, scheduled_at_round }) => {
                let state = self.as_mut();
                if scheduled_at_level == state.tb_state.level().unwrap_or(1) && scheduled_at_round == state.tb_state.round().unwrap_or(0) {
                    let mut tb_actions = vec![];
                    state.tb_state.handle(&state.tb_config, tb::Event::Timeout, &mut tb_actions);
                    state.handle_tb_actions(tb_actions);
                }
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
    fn handle_tb_actions(&mut self, tb_actions: Vec<tb::Action>) {
        for tb_action in tb_actions {
            match tb_action {
                tb::Action::ScheduleTimeout(deadline) => {
                    self.actions
                        .push(BakerAction::ScheduleTimeout(ScheduleTimeoutAction {
                            deadline,
                        }));
                }
                tb::Action::Propose(block, _, _) => {
                    self.propose(block);
                }
                tb::Action::Preendorse(branch, op) => {
                    self.pre_vote(branch, op);
                }
                tb::Action::Endorse(branch, op) => {
                    self.vote(branch, op);
                }
                // TODO: log
                _ => (),
            }
        }
    }

    fn pre_vote(&mut self, pred_hash: hash::BlockHash, block_id: tb::BlockId) {
        let slot = self
            .tb_config
            .map
            .delegates
            .get(&block_id.level)
            .and_then(|v| v.get(&self.this))
            .and_then(|v| v.0.first());
        let slot = match slot {
            Some(s) => *s,
            None => return,
        };
        let preendorsement = InlinedPreendorsement {
            branch: pred_hash.into(),
            operations: InlinedPreendorsementContents::Preendorsement(
                InlinedPreendorsementVariant {
                    slot,
                    level: block_id.level,
                    round: block_id.round,
                    block_payload_hash: block_id.block_payload_hash.into(),
                },
            ),
            signature: Signature(vec![0x55; 64]),
        };
        self.actions
            .push(BakerAction::PreVote(PreVoteAction { op: preendorsement }));
    }

    fn vote(&mut self, pred_hash: hash::BlockHash, block_id: tb::BlockId) {
        let slot = self
            .tb_config
            .map
            .delegates
            .get(&block_id.level)
            .and_then(|v| v.get(&self.this))
            .and_then(|v| v.0.first());
        let slot = match slot {
            Some(s) => *s,
            None => return,
        };
        let endorsement = InlinedEndorsement {
            branch: pred_hash.into(),
            operations: InlinedEndorsementMempoolContents::Endorsement(
                InlinedEndorsementMempoolContentsEndorsementVariant {
                    slot,
                    level: block_id.level,
                    round: block_id.round,
                    block_payload_hash: block_id.block_payload_hash.into(),
                },
            ),
            signature: Signature(vec![0x55; 64]),
        };
        self.actions
            .push(BakerAction::Vote(VoteAction { op: endorsement }));
    }

    fn propose(&mut self, block: tb::Block) {
        let payload = match block.payload {
            Some(v) => v,
            None => return,
        };

        let predecessor_hash = block.pred_hash;
        let endorsements = payload
            .cer
            .map(|q| q.votes.ids)
            .into_iter()
            .flatten()
            .filter_map(|(id, op)| {
                let slot = self
                    .tb_config
                    .map
                    .slot(op.contents.level, &id)
                    .unwrap_or(u16::MAX);
                Some(tb::OperationSimple::Endorsement(
                    op.map(|block_id| tb::ConsensusBody::new(block_id, slot)),
                ))
            });
        let preendorsements = payload
            .pre_cer
            .map(|q| q.votes.ids)
            .into_iter()
            .flatten()
            .filter_map(|(id, op)| {
                let slot = self
                    .tb_config
                    .map
                    .slot(op.contents.level, &id)
                    .unwrap_or(u16::MAX);
                Some(tb::OperationSimple::Preendorsement(
                    op.map(|block_id| tb::ConsensusBody::new(block_id, slot)),
                ))
            });
        let mut operations = [
            endorsements.chain(preendorsements).collect::<Vec<_>>(),
            vec![],
            vec![],
            vec![],
        ];
        #[cfg(feature = "testing-mock")]
        let _ = &mut operations;
        let mut hashes = BTreeSet::new();
        for mut op in payload.operations {
            if let Some(hash) = op.hash() {
                if !hashes.insert(hash.clone()) {
                    continue;
                }
            }
            op.strip_signature();
            match &op {
                tb::OperationSimple::Preendorsement(_) | tb::OperationSimple::Endorsement(_) => {
                    let description = format!("unexpected consensus operation {op:?}");
                    self.actions
                        .push(BakerAction::LogWarning(LogWarningAction { description }));
                }
                #[cfg(not(feature = "testing-mock"))]
                tb::OperationSimple::Votes(_) => operations[1].push(op),
                #[cfg(not(feature = "testing-mock"))]
                tb::OperationSimple::Anonymous(_) => operations[2].push(op),
                #[cfg(not(feature = "testing-mock"))]
                tb::OperationSimple::Managers(_) => operations[3].push(op),
            }
        }
        let payload_round = payload.payload_round;
        let seed_nonce_hash = self.nonces.gen_nonce(block.level);
        let timestamp = block.time_header.timestamp.unix_epoch().as_secs() as i64;

        self.actions.push(BakerAction::Propose(ProposeAction {
            payload_round,
            seed_nonce_hash,
            predecessor_hash,
            operations,
            timestamp,
            round: block.time_header.round,
            level: block.level,
        }))
    }
}

impl tb::ProposerMap for SlotsInfo {
    fn proposer(&self, level: i32, round: i32) -> Option<(i32, hash::ContractTz1Hash)> {
        let c = self.committee_size as i32;
        let m = round / c;
        let round = round % c;
        self.ours
            .iter()
            .filter_map(|our| {
                let Slots(slots) = self.delegates.get(&level)?.get(our)?;

                let this_loop = slots
                    .iter()
                    .find(|c| **c >= round as u16)
                    .map(|r| ((*r as i32) + m * c, our.clone()));

                let next_loop = slots
                    .iter()
                    .take_while(|c| **c < round as u16)
                    .next()
                    .map(|r| ((*r as i32) + (m + 1) * c, our.clone()));

                this_loop.or(next_loop)
            })
            .min_by(|(a, _), (b, _)| a.cmp(b))
    }
}

impl SlotsInfo {
    fn validator(&self, level: i32, slot: u16) -> Option<(hash::ContractTz1Hash, u32)> {
        let i = self.delegates.get(&level)?;
        let (id, s) = i.iter().find(|&(_, v)| v.0.first() == Some(&slot))?;
        Some((id.clone(), s.0.len() as u32))
    }

    fn slot(&self, level: i32, id: &hash::ContractTz1Hash) -> Option<u16> {
        self.delegates.get(&level)?.get(id)?.0.first().cloned()
    }
}

fn proposal(
    block: &Block,
    operations: Vec<Vec<tb::OperationSimple>>,
    tb_config: &tb::Config<tb::TimingLinearGrow, SlotsInfo>,
) -> tb::Block {
    tb::Block {
        pred_hash: block.predecessor.clone(),
        level: block.level,
        hash: block.hash.clone(),
        time_header: tb::TimeHeader {
            round: block.round,
            timestamp: tb::Timestamp::from_unix_epoch(Duration::from_secs(block.timestamp)),
        },
        payload: {
            if !block.transition {
                Some(tb::Payload {
                    hash: block.payload_hash.clone(),
                    payload_round: block.payload_round,
                    pre_cer: operations.first().and_then(|ops| {
                        let ops = ops
                            .iter()
                            .filter_map(|op| match op {
                                tb::OperationSimple::Preendorsement(v) => Some(v.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        let first = ops.first()?;
                        Some(tb::PreCertificate {
                            payload_hash: first.contents.block_payload_hash.clone(),
                            payload_round: first.contents.round,
                            votes: {
                                ops.into_iter()
                                    .filter_map(|op| {
                                        let (id, power) = tb_config
                                            .map
                                            .validator(op.contents.level, op.contents.slot)?;
                                        Some(tb::Vote {
                                            id,
                                            power,
                                            op: op.map(tb::ConsensusBody::block_id),
                                        })
                                    })
                                    .collect()
                            },
                        })
                    }),
                    cer: operations.first().map(|ops| tb::Certificate {
                        votes: {
                            ops.iter()
                                .filter_map(|op| match op {
                                    tb::OperationSimple::Endorsement(op) => {
                                        let (id, power) = tb_config
                                            .map
                                            .validator(op.contents.level, op.contents.slot)?;
                                        Some(tb::Vote {
                                            id,
                                            power,
                                            op: op.clone().map(tb::ConsensusBody::block_id),
                                        })
                                    }
                                    _ => None,
                                })
                                .collect()
                        },
                    }),
                    operations: operations.into_iter().skip(1).flatten().collect(),
                })
            } else {
                None
            }
        },
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(not(feature = "testing-mock"))]
    fn convert_proposal() {
        use crate::machine::state::SlotsInfo;

        use crate::tenderbake_new as tb;

        let map_str = include_str!("test_data/delegates.json");
        let ops_str = include_str!("test_data/operations.json");

        let operations = serde_json::from_str::<Vec<Vec<tb::OperationSimple>>>(ops_str).unwrap();
        let map = serde_json::from_str::<SlotsInfo>(map_str).unwrap();

        let cer = operations.first().map(|ops| tb::Certificate {
            votes: {
                ops.iter()
                    .filter_map(|op| match op {
                        tb::OperationSimple::Endorsement(op) => {
                            let (id, power) = map.validator(op.contents.level, op.contents.slot)?;
                            Some(tb::Vote {
                                id,
                                power,
                                op: op.clone().map(tb::ConsensusBody::block_id),
                            })
                        }
                        _ => None,
                    })
                    .collect()
            },
        });

        println!("{}", serde_json::to_string(&cer.unwrap()).unwrap());
    }
}
