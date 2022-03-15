// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fmt, time::Duration};

use derive_more::From;
use serde::{Deserialize, Serialize};

use redux_rs::EnablingCondition;

use crypto::hash::{BlockHash, ChainId, OperationHash};
use tezos_messages::{
    p2p::encoding::operation::DecodedOperation,
    protocol::proto_012::operation::{FullHeader, InlinedEndorsementMempoolContents, Operation},
};

use crate::{
    machine::state::State,
    rpc_client::{Constants, RpcError},
    types::{DelegateSlots, EndorsementUnsignedOperation, Proposal, Timestamp},
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
pub struct TimeoutScheduleAction {}

impl EnablingCondition<State> for TimeoutScheduleAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::Ready { .. })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TimeoutAction {
    pub now_timestamp: Duration,
}

impl EnablingCondition<State> for TimeoutAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::Ready { .. })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TimeoutDelayedAction {
    pub now_timestamp: Duration,
}

impl EnablingCondition<State> for TimeoutDelayedAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::Ready { .. })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewProposalAction {
    pub new_proposal: Proposal,
    pub delegate_slots: DelegateSlots,
    pub next_level_delegate_slots: DelegateSlots,
    pub now_timestamp: Timestamp,
}

impl EnablingCondition<State> for NewProposalAction {
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
        // match (self.operations.first(), state) {
        //     (Some(op), State::Ready { level_state, .. }) => {
        //         op.branch.eq(&level_state.latest_proposal.block.hash)
        //     }
        //     _ => false,
        // }
        // TODO: check operations
        matches!(state, State::Ready { .. })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InjectEndorsementInitAction {}

impl EnablingCondition<State> for InjectEndorsementInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match state {
            State::Ready {
                endorsement:
                    Some(EndorsementUnsignedOperation {
                        content: InlinedEndorsementMempoolContents::Endorsement(e),
                        ..
                    }),
                level_state,
                ..
            } => {
                // endorsement should be included in next block
                // after the block where consensus was observed
                e.level + 1 == level_state.current_level
            }
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InjectEndorsementSuccessAction {
    pub hash: OperationHash,
}

impl EnablingCondition<State> for InjectEndorsementSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PreapplyBlockInitAction {
    pub timestamp: i64,
}

impl EnablingCondition<State> for PreapplyBlockInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::Ready { block: Some(_), .. })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PreapplyBlockSuccessAction {
    pub header: FullHeader,
    pub operations: Vec<Vec<DecodedOperation>>,
}

impl EnablingCondition<State> for PreapplyBlockSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InjectBlockInitAction {
    pub header: FullHeader,
    pub operations: Vec<Vec<DecodedOperation>>,
}

impl EnablingCondition<State> for InjectBlockInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(state, State::Ready { block: Some(_), .. })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InjectBlockSuccessAction {
    pub hash: BlockHash,
}

impl EnablingCondition<State> for InjectBlockSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
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
    pub description: String,
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
    TimeoutSchedule(TimeoutScheduleAction),
    Timeout(TimeoutAction),
    TimeoutDelayed(TimeoutDelayedAction),
    NewProposal(NewProposalAction),
    InjectPreendorsementInit(InjectPreendorsementInitAction),
    InjectPreendorsementSuccess(InjectPreendorsementSuccessAction),
    NewOperationSeen(NewOperationSeenAction),
    InjectEndorsementInit(InjectEndorsementInitAction),
    InjectEndorsementSuccess(InjectEndorsementSuccessAction),
    PreapplyBlockInit(PreapplyBlockInitAction),
    PreapplyBlockSuccess(PreapplyBlockSuccessAction),
    InjectBlockInit(InjectBlockInitAction),
    InjectBlockSuccess(InjectBlockSuccessAction),

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
            Action::TimeoutSchedule(v) => fmt::Debug::fmt(v, f),
            Action::Timeout(v) => fmt::Debug::fmt(v, f),
            Action::TimeoutDelayed(v) => fmt::Debug::fmt(v, f),
            Action::NewProposal(v) => fmt::Debug::fmt(v, f),
            Action::InjectPreendorsementInit(v) => fmt::Debug::fmt(v, f),
            Action::InjectPreendorsementSuccess(v) => fmt::Debug::fmt(v, f),
            Action::NewOperationSeen(v) => fmt::Debug::fmt(v, f),
            Action::InjectEndorsementInit(v) => fmt::Debug::fmt(v, f),
            Action::InjectEndorsementSuccess(v) => fmt::Debug::fmt(v, f),
            Action::PreapplyBlockInit(v) => fmt::Debug::fmt(v, f),
            Action::PreapplyBlockSuccess(v) => fmt::Debug::fmt(v, f),
            Action::InjectBlockInit(v) => fmt::Debug::fmt(v, f),
            Action::InjectBlockSuccess(v) => fmt::Debug::fmt(v, f),

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
            Action::TimeoutSchedule(v) => v.is_enabled(state),
            Action::Timeout(v) => v.is_enabled(state),
            Action::TimeoutDelayed(v) => v.is_enabled(state),
            Action::NewProposal(v) => v.is_enabled(state),
            Action::InjectPreendorsementInit(v) => v.is_enabled(state),
            Action::InjectPreendorsementSuccess(v) => v.is_enabled(state),
            Action::NewOperationSeen(v) => v.is_enabled(state),
            Action::InjectEndorsementInit(v) => v.is_enabled(state),
            Action::InjectEndorsementSuccess(v) => v.is_enabled(state),
            Action::PreapplyBlockInit(v) => v.is_enabled(state),
            Action::PreapplyBlockSuccess(v) => v.is_enabled(state),
            Action::InjectBlockInit(v) => v.is_enabled(state),
            Action::InjectBlockSuccess(v) => v.is_enabled(state),

            Action::RecoverableError(v) => v.is_enabled(state),
            Action::UnrecoverableError(v) => v.is_enabled(state),
        }
    }
}
