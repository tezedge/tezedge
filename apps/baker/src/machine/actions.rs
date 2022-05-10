// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, sync::Arc};

use crypto::hash::{BlockHash, ContractTz1Hash};
use redux_rs::EnablingCondition;
use tezos_messages::protocol::proto_012::operation::{InlinedEndorsement, InlinedPreendorsement};

use crate::services::{
    client::{ProtocolBlockHeader, RpcError},
    event::{Block, OperationSimple},
};

use super::{state::Gathering, BakerState};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct RpcErrorAction {
    pub error: Arc<RpcError>,
}

impl<S> EnablingCondition<S> for RpcErrorAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct IdleEventAction {}

impl<S> EnablingCondition<S> for IdleEventAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct ProposalEventAction {
    pub block: Block,
}

impl<S> EnablingCondition<S> for ProposalEventAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct SlotsEventAction {
    pub level: i32,
    pub delegates: BTreeMap<ContractTz1Hash, Vec<u16>>,
}

impl<S> EnablingCondition<S> for SlotsEventAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        matches!(
            state.as_ref(),
            Some(BakerState::Gathering { gathering: Gathering::GetSlots(r), .. }) if r.is_pending(),
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct OperationsForBlockEventAction {
    pub block_hash: BlockHash,
    pub operations: Vec<Vec<OperationSimple>>,
}

impl<S> EnablingCondition<S> for OperationsForBlockEventAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        matches!(
            state.as_ref(),
            Some(BakerState::Gathering { gathering: Gathering::GetOperations(r), .. }) if r.is_pending(),
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct LiveBlocksEventAction {
    pub block_hash: BlockHash,
    pub live_blocks: Vec<BlockHash>,
}

impl<S> EnablingCondition<S> for LiveBlocksEventAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        matches!(
            state.as_ref(),
            Some(BakerState::Gathering { gathering: Gathering::GetLiveBlocks(r), .. }) if r.is_pending(),
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct OperationsEventAction {
    pub operations: Vec<OperationSimple>,
}

impl<S> EnablingCondition<S> for OperationsEventAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct TickEventAction {}

impl<S> EnablingCondition<S> for TickEventAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

// Inner actions

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct IdleAction {}

impl<S> EnablingCondition<S> for IdleAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct LogErrorAction {
    pub description: String,
}

impl<S> EnablingCondition<S> for LogErrorAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct LogWarningAction {
    pub description: String,
}

impl<S> EnablingCondition<S> for LogWarningAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct LogInfoAction {
    pub with_prefix: bool,
    pub description: String,
}

impl<S> EnablingCondition<S> for LogInfoAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

// TODO: split into many individual actions
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct LogTenderbakeAction {
    pub record: tenderbake::LogRecord,
}

impl<S> EnablingCondition<S> for LogTenderbakeAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct GetSlotsAction {
    pub level: i32,
}

impl<S> EnablingCondition<S> for GetSlotsAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct GetOperationsForBlockAction {
    pub block_hash: BlockHash,
}

impl<S> EnablingCondition<S> for GetOperationsForBlockAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct GetLiveBlocksAction {
    pub block_hash: BlockHash,
}

impl<S> EnablingCondition<S> for GetLiveBlocksAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct MonitorOperationsAction {}

impl<S> EnablingCondition<S> for MonitorOperationsAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct ScheduleTimeoutAction {
    pub deadline: tenderbake::Timestamp,
}

impl<S> EnablingCondition<S> for ScheduleTimeoutAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct RevealNonceAction {
    pub branch: BlockHash,
    pub level: i32,
    pub nonce: Vec<u8>,
}

impl<S> EnablingCondition<S> for RevealNonceAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct PreVoteAction {
    pub op: InlinedPreendorsement,
}

impl<S> EnablingCondition<S> for PreVoteAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct VoteAction {
    pub op: InlinedEndorsement,
}

impl<S> EnablingCondition<S> for VoteAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct ProposeAction {
    pub protocol_header: ProtocolBlockHeader,
    pub predecessor_hash: BlockHash,
    pub operations: [Vec<OperationSimple>; 4],
    pub timestamp: i64,
    pub round: i32,
}

impl<S> EnablingCondition<S> for ProposeAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[derive(Clone, Debug)]
pub enum BakerAction {
    // events
    RpcError(RpcErrorAction),
    IdleEvent(IdleEventAction),
    ProposalEvent(ProposalEventAction),
    SlotsEvent(SlotsEventAction),
    OperationsForBlockEvent(OperationsForBlockEventAction),
    LiveBlocksEvent(LiveBlocksEventAction),
    OperationsEvent(OperationsEventAction),
    TickEvent(TickEventAction),
    // inner actions
    Idle(IdleAction),
    LogError(LogErrorAction),
    LogWarning(LogWarningAction),
    LogInfo(LogInfoAction),
    LogTenderbake(LogTenderbakeAction),
    GetSlots(GetSlotsAction),
    GetOperationsForBlock(GetOperationsForBlockAction),
    GetLiveBlocks(GetLiveBlocksAction),
    MonitorOperations(MonitorOperationsAction),
    ScheduleTimeout(ScheduleTimeoutAction),
    RevealNonce(RevealNonceAction),
    PreVote(PreVoteAction),
    Vote(VoteAction),
    Propose(ProposeAction),
}

impl BakerAction {
    pub fn is_event(&self) -> bool {
        matches!(
            self,
            BakerAction::RpcError(_)
                | BakerAction::IdleEvent(_)
                | BakerAction::ProposalEvent(_)
                | BakerAction::SlotsEvent(_)
                | BakerAction::OperationsForBlockEvent(_)
                | BakerAction::LiveBlocksEvent(_)
                | BakerAction::OperationsEvent(_)
                | BakerAction::TickEvent(_)
        )
    }
}

impl From<BakerAction> for Action {
    fn from(v: BakerAction) -> Self {
        Action(Some(v))
    }
}

pub struct Action(pub Option<BakerAction>);

impl<S> EnablingCondition<S> for BakerAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        match &self {
            BakerAction::RpcError(action) => action.is_enabled(state),
            BakerAction::IdleEvent(action) => action.is_enabled(state),
            BakerAction::ProposalEvent(action) => action.is_enabled(state),
            BakerAction::SlotsEvent(action) => action.is_enabled(state),
            BakerAction::OperationsForBlockEvent(action) => action.is_enabled(state),
            BakerAction::LiveBlocksEvent(action) => action.is_enabled(state),
            BakerAction::OperationsEvent(action) => action.is_enabled(state),
            BakerAction::TickEvent(action) => action.is_enabled(state),
            BakerAction::Idle(action) => action.is_enabled(state),
            BakerAction::LogError(action) => action.is_enabled(state),
            BakerAction::LogWarning(action) => action.is_enabled(state),
            BakerAction::LogInfo(action) => action.is_enabled(state),
            BakerAction::LogTenderbake(action) => action.is_enabled(state),
            BakerAction::GetSlots(action) => action.is_enabled(state),
            BakerAction::GetOperationsForBlock(action) => action.is_enabled(state),
            BakerAction::GetLiveBlocks(action) => action.is_enabled(state),
            BakerAction::MonitorOperations(action) => action.is_enabled(state),
            BakerAction::ScheduleTimeout(action) => action.is_enabled(state),
            BakerAction::RevealNonce(action) => action.is_enabled(state),
            BakerAction::PreVote(action) => action.is_enabled(state),
            BakerAction::Vote(action) => action.is_enabled(state),
            BakerAction::Propose(action) => action.is_enabled(state),
        }
    }
}

impl AsRef<Option<BakerAction>> for Action {
    fn as_ref(&self) -> &Option<BakerAction> {
        &self.0
    }
}
