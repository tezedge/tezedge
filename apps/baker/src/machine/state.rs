// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, mem,
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, BlockPayloadHash, ChainId, ContractTz1Hash, Signature};
use tenderbake as tb;
use tezos_messages::protocol::proto_012::operation::{
    EndorsementOperation, InlinedEndorsement, InlinedEndorsementMempoolContents,
    InlinedEndorsementMempoolContentsEndorsementVariant, InlinedPreendorsement,
    InlinedPreendorsementContents, InlinedPreendorsementVariant,
};

use crate::services::{
    client::{Constants, LiquidityBakingToggleVote, Protocol},
    event::{Block, OperationKind, OperationSimple, Slots},
    EventWithTime,
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
    pub delegates: BTreeMap<i32, BTreeMap<ContractTz1Hash, Slots>>,
}

#[derive(Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
pub enum Gathering {
    GetCornerSlots(Request<i32, BTreeMap<ContractTz1Hash, Slots>, String>),
    // for some `level: i32` we request a collection of public key hash
    // and corresponding slots
    GetSlots(Request<i32, BTreeMap<ContractTz1Hash, Slots>, String>),
    // for some `BlockHash` we request its operations
    GetOperations(Request<BlockHash, Vec<Vec<OperationSimple>>, String>),
    // for some `BlockHash` we request a list of live blocks
    GetLiveBlocks(Request<BlockHash, Vec<BlockHash>, String>),
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
    pub this: ContractTz1Hash,
    // cycle state
    pub nonces: CycleNonce,
    // live blocks
    #[serde(default)]
    pub live_blocks_base: Option<BlockHash>,
    pub live_blocks: Vec<BlockHash>,
    // operations in this proposal
    pub operations: Vec<Vec<OperationSimple>>,
    #[serde(default)]
    pub new_operations: Vec<OperationSimple>,
    // tenderbake machine
    pub tb_config: tb::Config<tb::TimingLinearGrow, SlotsInfo>,
    pub tb_state: tb::Machine<ContractTz1Hash, OperationSimple>,

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
        this: ContractTz1Hash,
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
            live_blocks_base: None,
            live_blocks: Vec::new(),
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
                        state.live_blocks_base = Some(id);
                        state.live_blocks = live_blocks;
                        state.actions.push(BakerAction::Idle(IdleAction {}));
                        BakerState::HaveBlock { state, current_block }
                    },
                    BakerState::HaveBlock { mut state, current_block } => {
                        state.actions.push(BakerAction::MonitorOperations(MonitorOperationsAction {}));
                        let operations = mem::take(&mut state.operations);
                        let proposal = Box::new(proposal(&current_block, operations, &state.tb_config));
                        let (tb_actions, records) = state.tb_state.handle(&state.tb_config, tb::Event::Proposal(proposal, now));
                        state.actions.extend(records.into_iter().map(|record| {
                            BakerAction::LogTenderbake(LogTenderbakeAction { record })
                        }));
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
                    match op.kind() {
                        None => {
                            let description = format!("unclassified operation {op:?}");
                            state.actions.push(BakerAction::LogError(LogErrorAction { description }));
                        }
                        Some(OperationKind::Preendorsement(content)) => {
                            if let Some(validator) =
                                state
                                    .tb_config
                                    .map
                                    .validator(content.level, content.slot, op)
                            {
                                let event = tb::Event::Preendorsed(block_id(&content), validator, now);
                                let (tb_actions, records) =
                                    state.tb_state.handle(&state.tb_config, event);
                                state.actions.extend(records.into_iter().map(|record| {
                                    BakerAction::LogTenderbake(LogTenderbakeAction { record })
                                }));
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
                                let event = tb::Event::Endorsed(block_id(&content), validator, now);
                                let (tb_actions, records) =
                                    state.tb_state.handle(&state.tb_config, event);
                                state.actions.extend(records.into_iter().map(|record| {
                                    BakerAction::LogTenderbake(LogTenderbakeAction { record })
                                }));
                                state.handle_tb_actions(tb_actions);
                            }
                        }
                        Some(_) => {
                            if state.live_blocks_base != Some(state.tb_state.hash().as_ref().unwrap().clone()) {
                                state.new_operations.push(op);
                                continue;
                            }
                            for op in mem::take(&mut state.new_operations).into_iter().chain(std::iter::once(op)) {
                                // the operation does not belong to live_blocks
                                if !state.live_blocks.contains(&op.branch) {
                                    let description = format!("the op is outdated {op:?}");
                                    state.actions.push(BakerAction::LogWarning(LogWarningAction { description }));
                                    // state.ahead_ops.entry(op.branch.clone()).or_default().push(op);
                                    continue;
                                };
                                state
                                    .tb_state
                                    .handle(&state.tb_config, tb::Event::Operation(op));
                            }
                        }
                    }
                }
                self
            }
            BakerAction::TickEvent(TickEventAction { scheduled_at_level, scheduled_at_round }) => {
                let state = self.as_mut();
                if scheduled_at_level == state.tb_state.level().unwrap_or(1) && scheduled_at_round == state.tb_state.round().unwrap_or(0) {
                    let (tb_actions, records) =
                        state.tb_state.handle(&state.tb_config, tb::Event::Timeout);
                    state.actions.extend(records.into_iter().map(|record| {
                        BakerAction::LogTenderbake(LogTenderbakeAction { record })
                    }));
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
    fn handle_tb_actions(&mut self, tb_actions: Vec<tb::Action<ContractTz1Hash, OperationSimple>>) {
        for tb_action in tb_actions {
            match tb_action {
                tb::Action::ScheduleTimeout(deadline) => {
                    self.actions
                        .push(BakerAction::ScheduleTimeout(ScheduleTimeoutAction {
                            deadline,
                        }));
                }
                tb::Action::Propose(block, _, _) => {
                    self.propose(*block);
                }
                tb::Action::Preendorse {
                    pred_hash,
                    block_id,
                } => {
                    self.pre_vote(pred_hash, block_id);
                }
                tb::Action::Endorse {
                    pred_hash,
                    block_id,
                } => {
                    self.vote(pred_hash, block_id);
                }
            }
        }
    }

    fn pre_vote(&mut self, pred_hash: BlockHash, block_id: tb::BlockId) {
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
            branch: BlockHash(pred_hash.0.to_vec()),
            operations: InlinedPreendorsementContents::Preendorsement(
                InlinedPreendorsementVariant {
                    slot,
                    level: block_id.level,
                    round: block_id.round,
                    block_payload_hash: BlockPayloadHash(block_id.payload_hash.0.to_vec()),
                },
            ),
            signature: Signature(vec![0x55; 64]),
        };
        self.actions
            .push(BakerAction::PreVote(PreVoteAction { op: preendorsement }));
    }

    fn vote(&mut self, pred_hash: BlockHash, block_id: tb::BlockId) {
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
            branch: BlockHash(pred_hash.0.to_vec()),
            operations: InlinedEndorsementMempoolContents::Endorsement(
                InlinedEndorsementMempoolContentsEndorsementVariant {
                    slot,
                    level: block_id.level,
                    round: block_id.round,
                    block_payload_hash: BlockPayloadHash(block_id.payload_hash.0.to_vec()),
                },
            ),
            signature: Signature(vec![0x55; 64]),
        };
        self.actions
            .push(BakerAction::Vote(VoteAction { op: endorsement }));
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
                    let description = format!("unclassified operation {op:?}");
                    self.actions
                        .push(BakerAction::LogWarning(LogWarningAction { description }));
                }
                Some(OperationKind::Endorsement(_) | OperationKind::Preendorsement(_)) => {
                    let description = format!("unexpected consensus operation {op:?}");
                    self.actions
                        .push(BakerAction::LogWarning(LogWarningAction { description }));
                }
                Some(OperationKind::Votes) => operations[1].push(op),
                Some(OperationKind::Anonymous) => {
                    let mut op = op;
                    if op.signature.is_none() {
                        op.signature = Some(Signature(vec![0x55; 64]));
                    }
                    operations[2].push(op)
                }
                Some(OperationKind::Managers) => operations[3].push(op),
            }
        }
        let payload_round = payload.payload_round;
        let seed_nonce_hash = self.nonces.gen_nonce(block.level);
        let timestamp = block.time_header.timestamp.unix_epoch.as_secs() as i64;

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
    type Id = ContractTz1Hash;

    fn proposer(&self, level: i32, round: i32) -> Option<(i32, Self::Id)> {
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
    fn validator(
        &self,
        level: i32,
        slot: u16,
        operation: OperationSimple,
    ) -> Option<tb::Validator<ContractTz1Hash, OperationSimple>> {
        let i = self.delegates.get(&level)?;
        let (id, s) = i.iter().find(|&(_, v)| v.0.first() == Some(&slot))?;
        Some(tb::Validator {
            id: id.clone(),
            power: s.0.len() as u32,
            operation,
        })
    }
}

fn block_id(content: &EndorsementOperation) -> tb::BlockId {
    tb::BlockId {
        level: content.level,
        round: content.round,
        payload_hash: content.block_payload_hash.clone(),
    }
}

fn proposal(
    block: &Block,
    operations: Vec<Vec<OperationSimple>>,
    tb_config: &tb::Config<tb::TimingLinearGrow, SlotsInfo>,
) -> tb::Block<ContractTz1Hash, OperationSimple> {
    tb::Block {
        pred_hash: block.predecessor.clone(),
        level: block.level,
        hash: block.hash.clone(),
        time_header: tb::TimeHeader {
            round: block.round,
            timestamp: tb::Timestamp {
                unix_epoch: Duration::from_secs(block.timestamp),
            },
        },
        payload: {
            if !block.transition {
                Some(tb::Payload {
                    hash: block.payload_hash.clone(),
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
                            payload_hash: first.block_payload_hash.clone(),
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
                    cer: operations.first().map(|ops| tb::Certificate {
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
    use crate::{
        machine::state::SlotsInfo,
        services::event::{OperationKind, OperationSimple},
    };

    use tenderbake as tb;

    #[test]
    fn convert_proposal() {
        let map_str = include_str!("test_data/delegates.json");
        let ops_str = include_str!("test_data/operations.json");

        let operations = serde_json::from_str::<Vec<Vec<OperationSimple>>>(ops_str).unwrap();
        let map = serde_json::from_str::<SlotsInfo>(map_str).unwrap();

        let cer = operations.first().map(|ops| tb::Certificate {
            votes: {
                ops.iter()
                    .filter_map(|op| match op.kind()? {
                        OperationKind::Endorsement(v) => map.validator(v.level, v.slot, op.clone()),
                        _ => None,
                    })
                    .collect()
            },
        });

        println!("{}", serde_json::to_string(&cer.unwrap()).unwrap());
    }
}
