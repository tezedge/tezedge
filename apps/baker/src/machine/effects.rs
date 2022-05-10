// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithMeta, Store, TimeService};

use crate::{services::BakerService, ActionInner};

use super::{actions::*, state::BakerState};

pub fn baker_effects<S, Srv, A>(store: &mut Store<S, Srv, A>, action: &ActionWithMeta<A>)
where
    S: AsRef<Option<BakerState>>,
    Srv: TimeService + BakerService,
    A: AsRef<Option<BakerAction>> + From<BakerAction>,
{
    if let Some(baker_state) = store.state.get().as_ref() {
        let to_dispatch = baker_state.as_ref().actions.clone();
        for action in to_dispatch {
            store.dispatch(action);
        }
    }

    match action.action.as_ref() {
        // not our action
        None => (),
        // don't handle events here
        Some(
            BakerAction::RpcError(_)
            | BakerAction::IdleEvent(_)
            | BakerAction::ProposalEvent(_)
            | BakerAction::SlotsEvent(_)
            | BakerAction::OperationsForBlockEvent(_)
            | BakerAction::LiveBlocksEvent(_)
            | BakerAction::OperationsEvent(_)
            | BakerAction::TickEvent(_),
        ) => (),
        Some(BakerAction::Idle(IdleAction {})) => {
            store.dispatch(BakerAction::IdleEvent(IdleEventAction {}));
        }
        Some(BakerAction::LogError(LogErrorAction { description })) => {
            store
                .service
                .execute(&ActionInner::LogError(description.clone()));
        }
        Some(BakerAction::LogWarning(LogWarningAction { description })) => {
            store
                .service
                .execute(&ActionInner::LogWarning(description.clone()));
        }
        Some(BakerAction::LogInfo(LogInfoAction {
            with_prefix,
            description,
        })) => {
            store.service.execute(&ActionInner::LogInfo {
                with_prefix: *with_prefix,
                description: description.clone(),
            });
        }
        Some(BakerAction::LogTenderbake(LogTenderbakeAction { record })) => {
            store.service.execute(&ActionInner::LogTb(record.clone()));
        }
        Some(BakerAction::GetSlots(GetSlotsAction { level })) => {
            store
                .service
                .execute(&ActionInner::GetSlots { level: *level });
        }
        Some(BakerAction::GetOperationsForBlock(GetOperationsForBlockAction { block_hash })) => {
            store.service.execute(&ActionInner::GetOperationsForBlock {
                block_hash: block_hash.clone(),
            });
        }
        Some(BakerAction::GetLiveBlocks(GetLiveBlocksAction { block_hash })) => {
            store.service.execute(&ActionInner::GetLiveBlocks {
                block_hash: block_hash.clone(),
            });
        }
        Some(BakerAction::MonitorOperations(MonitorOperationsAction {})) => {
            store.service.execute(&ActionInner::MonitorOperations);
        }
        Some(BakerAction::ScheduleTimeout(ScheduleTimeoutAction { deadline })) => {
            store
                .service
                .execute(&ActionInner::ScheduleTimeout(*deadline));
        }
        Some(BakerAction::RevealNonce(RevealNonceAction {
            branch,
            level,
            nonce,
        })) => {
            store.service.execute(&ActionInner::RevealNonce {
                chain_id: store
                    .state
                    .get()
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .chain_id
                    .clone(),
                branch: branch.clone(),
                level: *level,
                nonce: nonce.clone(),
            });
        }
        Some(BakerAction::PreVote(PreVoteAction { op })) => {
            let chain_id = store
                .state
                .get()
                .as_ref()
                .as_ref()
                .unwrap()
                .as_ref()
                .chain_id
                .clone();
            store
                .service
                .execute(&ActionInner::PreVote(chain_id, op.clone()));
        }
        Some(BakerAction::Vote(VoteAction { op })) => {
            let chain_id = store
                .state
                .get()
                .as_ref()
                .as_ref()
                .unwrap()
                .as_ref()
                .chain_id
                .clone();
            store
                .service
                .execute(&ActionInner::Vote(chain_id, op.clone()));
        }
        Some(BakerAction::Propose(ProposeAction {
            protocol_header,
            predecessor_hash,
            operations,
            timestamp,
            round,
        })) => {
            let state = store.state.get().as_ref().as_ref().unwrap().as_ref();
            store.service.execute(&ActionInner::Propose {
                chain_id: state.chain_id.clone(),
                proof_of_work_threshold: state.proof_of_work_threshold,
                protocol_header: protocol_header.clone(),
                predecessor_hash: predecessor_hash.clone(),
                operations: operations.clone(),
                timestamp: *timestamp,
                round: *round,
            });
        }
    }
}
