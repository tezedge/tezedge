// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, sync::Arc};

use crypto::hash::{ContractTz1Hash, BlockHash};
use redux_rs::EnablingCondition;

use crate::services::{event::{Block, OperationSimple}, client::RpcError};

use super::{BakerState, state::Gathering};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone)]
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
#[derive(Clone)]
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
#[derive(Clone)]
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
#[derive(Clone)]
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
#[derive(Clone)]
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
#[derive(Clone)]
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
#[derive(Clone)]
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
#[derive(Clone)]
pub struct TickEventAction {}

impl<S> EnablingCondition<S> for TickEventAction
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        state.as_ref().is_some()
    }
}

#[derive(Clone)]
pub enum BakerAction {
    RpcError(RpcErrorAction),
    IdleEvent(IdleEventAction),
    ProposalEvent(ProposalEventAction),
    SlotsEvent(SlotsEventAction),
    OperationsForBlockEvent(OperationsForBlockEventAction),
    LiveBlocksEvent(LiveBlocksEventAction),
    OperationsEvent(OperationsEventAction),
    TickEvent(TickEventAction),
}

pub struct Action(pub Option<BakerAction>);

impl<S> EnablingCondition<S> for Action
where
    S: AsRef<Option<BakerState>>,
{
    fn is_enabled(&self, state: &S) -> bool {
        match &self.0 {
            None => false,
            Some(BakerAction::RpcError(action)) => action.is_enabled(state),
            Some(BakerAction::IdleEvent(action)) => action.is_enabled(state),
            Some(BakerAction::ProposalEvent(action)) => action.is_enabled(state),
            Some(BakerAction::SlotsEvent(action)) => action.is_enabled(state),
            Some(BakerAction::OperationsForBlockEvent(action)) => action.is_enabled(state),
            Some(BakerAction::LiveBlocksEvent(action)) => action.is_enabled(state),
            Some(BakerAction::OperationsEvent(action)) => action.is_enabled(state),
            Some(BakerAction::TickEvent(action)) => action.is_enabled(state),
        }
    }
}

impl AsRef<Option<BakerAction>> for Action {
    fn as_ref(&self) -> &Option<BakerAction> {
        &self.0
    }
}
