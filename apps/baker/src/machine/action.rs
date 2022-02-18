// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;

use derive_more::From;
use serde::{Deserialize, Serialize};

use redux_rs::EnablingCondition;

use crypto::hash::{ChainId, OperationHash};
use tezos_messages::protocol::proto_012::operation::Operation;

use crate::{
    machine::state::State,
    rpc_client::{Constants, RpcError},
    types::{DelegateSlots, Proposal},
};

#[derive(Debug)]
pub struct GetChainIdInitAction {}

impl EnablingCondition<State> for GetChainIdInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::Initial)
    }
}

#[derive(Debug)]
pub struct GetChainIdSuccessAction {
    pub chain_id: ChainId,
}

impl EnablingCondition<State> for GetChainIdSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::Initial)
    }
}

#[derive(Debug)]
pub struct GetChainIdErrorAction {
    pub error: RpcError,
}

impl EnablingCondition<State> for GetChainIdErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::Initial)
    }
}

#[derive(Debug)]
pub struct GetConstantsInitAction {}

impl EnablingCondition<State> for GetConstantsInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::GotChainId(_))
    }
}

#[derive(Debug)]
pub struct GetConstantsSuccessAction {
    pub constants: Constants,
}

impl EnablingCondition<State> for GetConstantsSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::GotChainId(_))
    }
}

#[derive(Debug)]
pub struct GetConstantsErrorAction {
    pub error: RpcError,
}

impl EnablingCondition<State> for GetConstantsErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::GotChainId(_))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewProposal {
    pub new_proposal: Proposal,
    pub delegate_slots: DelegateSlots,
    pub next_level_delegate_slots: DelegateSlots,
    pub now_timestamp: i64,
}

impl EnablingCondition<State> for NewProposal {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::GotConstants(_) | State::Ready { .. })
    }
}

#[derive(Debug)]
pub struct SignPreendorsementAction {}

impl EnablingCondition<State> for SignPreendorsementAction {
    fn is_enabled(&self, state: &State) -> bool {
        match state {
            State::Ready { level_state, .. } => level_state.delegate_slots.slot.is_some(),
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct InjectPreendorsementInitAction {}

impl EnablingCondition<State> for InjectPreendorsementInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match state {
            State::Ready { preendorsement, .. } => preendorsement.is_some(),
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct InjectPreendorsementSuccessAction {
    pub hash: OperationHash,
}

impl EnablingCondition<State> for InjectPreendorsementSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        InjectPreendorsementInitAction {}.is_enabled(state)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewOperationSeenAction {
    pub operations: Vec<Operation>,
}

impl EnablingCondition<State> for NewOperationSeenAction {
    fn is_enabled(&self, state: &State) -> bool {
        match (self.operations.first(), state) {
            (Some(op), State::Ready { level_state, .. }) => {
                op.branch.eq(&level_state.latest_proposal.block.hash)
            }
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct RecoverableErrorAction {
    pub description: String,
}

impl EnablingCondition<State> for RecoverableErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Debug)]
pub struct UnrecoverableErrorAction {
    pub rpc_error: RpcError,
}

impl EnablingCondition<State> for UnrecoverableErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(From)]
pub enum Action {
    GetChainIdInit(GetChainIdInitAction),
    GetChainIdSuccess(GetChainIdSuccessAction),
    GetChainIdError(GetChainIdErrorAction),
    GetConstantsInit(GetConstantsInitAction),
    GetConstantsSuccess(GetConstantsSuccessAction),
    GetConstantsError(GetConstantsErrorAction),
    NewProposal(NewProposal),
    InjectPreendorsementInit(InjectPreendorsementInitAction),
    InjectPreendorsementSuccess(InjectPreendorsementSuccessAction),
    NewOperationSeen(NewOperationSeenAction),

    RecoverableError(RecoverableErrorAction),
    UnrecoverableError(UnrecoverableErrorAction),
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::GetChainIdInit(v) => fmt::Debug::fmt(v, f),
            Action::GetChainIdSuccess(v) => fmt::Debug::fmt(v, f),
            Action::GetChainIdError(v) => fmt::Debug::fmt(v, f),
            Action::GetConstantsInit(v) => fmt::Debug::fmt(v, f),
            Action::GetConstantsSuccess(v) => fmt::Debug::fmt(v, f),
            Action::GetConstantsError(v) => fmt::Debug::fmt(v, f),
            Action::NewProposal(v) => fmt::Debug::fmt(v, f),
            Action::InjectPreendorsementInit(v) => fmt::Debug::fmt(v, f),
            Action::InjectPreendorsementSuccess(v) => fmt::Debug::fmt(v, f),
            Action::NewOperationSeen(v) => fmt::Debug::fmt(v, f),

            Action::RecoverableError(v) => fmt::Debug::fmt(v, f),
            Action::UnrecoverableError(v) => fmt::Debug::fmt(v, f),
        }
    }
}

impl EnablingCondition<State> for Action {
    fn is_enabled(&self, state: &State) -> bool {
        match self {
            Action::GetChainIdInit(v) => v.is_enabled(state),
            Action::GetChainIdSuccess(v) => v.is_enabled(state),
            Action::GetChainIdError(v) => v.is_enabled(state),
            Action::GetConstantsInit(v) => v.is_enabled(state),
            Action::GetConstantsSuccess(v) => v.is_enabled(state),
            Action::GetConstantsError(v) => v.is_enabled(state),
            Action::NewProposal(v) => v.is_enabled(state),
            Action::InjectPreendorsementInit(v) => v.is_enabled(state),
            Action::InjectPreendorsementSuccess(v) => v.is_enabled(state),
            Action::NewOperationSeen(v) => v.is_enabled(state),

            Action::RecoverableError(v) => v.is_enabled(state),
            Action::UnrecoverableError(v) => v.is_enabled(state),
        }
    }
}
