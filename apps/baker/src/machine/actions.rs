// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, rc::Rc};

use either::Either;

use crypto::hash::{ContractTz1Hash, BlockHash};
use redux_rs::EnablingCondition;

use crate::services::{event::{Event, Block, OperationSimple}, ActionInner, client::RpcError};

use super::{BakerState, state::Gathering};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone)]
pub struct RpcErrorAction {
    error: Rc<RpcError>,
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

impl From<BakerAction> for Either<Result<Event, Rc<RpcError>>, ActionInner> {
    fn from(v: BakerAction) -> Self {
        match v {
            BakerAction::RpcError(RpcErrorAction { error }) => Either::Left(Err(error)),
            BakerAction::IdleEvent(IdleEventAction {}) => Either::Left(Ok(Event::Idle)),
            BakerAction::ProposalEvent(ProposalEventAction { block }) => {
                Either::Left(Ok(Event::Block(block)))
            },
            BakerAction::SlotsEvent(SlotsEventAction { level, delegates }) => {
                Either::Left(Ok(Event::Slots { level, delegates }))
            },
            BakerAction::OperationsForBlockEvent(act) => {
                let OperationsForBlockEventAction { block_hash, operations } = act;
                Either::Left(Ok(Event::OperationsForBlock { block_hash, operations }))
            },
            BakerAction::LiveBlocksEvent(act) => {
                let LiveBlocksEventAction { block_hash, live_blocks } = act;
                Either::Left(Ok(Event::LiveBlocks { block_hash, live_blocks }))
            },
            BakerAction::OperationsEvent(OperationsEventAction { operations }) => {
                Either::Left(Ok(Event::Operations(operations)))
            },
            BakerAction::TickEvent(TickEventAction {}) => Either::Left(Ok(Event::Tick)),
        }
    }
}

impl From<Result<Event, Rc<RpcError>>> for BakerAction {
    fn from(v: Result<Event, Rc<RpcError>>) -> Self {
        match v {
            Err(error) => BakerAction::RpcError(RpcErrorAction { error }),
            Ok(Event::Idle) => BakerAction::IdleEvent(IdleEventAction {}),
            Ok(Event::Block(block)) => BakerAction::ProposalEvent(ProposalEventAction { block }),
            Ok(Event::Slots { level, delegates }) => {
                BakerAction::SlotsEvent(SlotsEventAction { level, delegates })
            }
            Ok(Event::OperationsForBlock { block_hash, operations }) => {
                let act = OperationsForBlockEventAction { block_hash, operations };
                BakerAction::OperationsForBlockEvent(act)
            }
            Ok(Event::LiveBlocks { block_hash, live_blocks }) => {
                let act = LiveBlocksEventAction { block_hash, live_blocks };
                BakerAction::LiveBlocksEvent(act)
            }
            Ok(Event::Operations(operations)) => {
                BakerAction::OperationsEvent(OperationsEventAction { operations })
            }
            Ok(Event::Tick) => BakerAction::TickEvent(TickEventAction {}),
        }
    }
}
