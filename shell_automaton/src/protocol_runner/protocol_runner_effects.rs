// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::init::context::{
    ProtocolRunnerInitContextErrorAction, ProtocolRunnerInitContextState,
    ProtocolRunnerInitContextSuccessAction,
};
use crate::protocol_runner::init::runtime::{
    ProtocolRunnerInitRuntimeErrorAction, ProtocolRunnerInitRuntimeState,
    ProtocolRunnerInitRuntimeSuccessAction,
};
use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::spawn_server::{
    ProtocolRunnerSpawnServerErrorAction, ProtocolRunnerSpawnServerState,
    ProtocolRunnerSpawnServerSuccessAction,
};
use crate::protocol_runner::ProtocolRunnerResponseAction;
use crate::service::protocol_runner_service::ProtocolRunnerResult;
use crate::service::ProtocolRunnerService;
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::init::ProtocolRunnerInitAction;
use super::spawn_server::ProtocolRunnerSpawnServerInitAction;
use super::{
    ProtocolRunnerNotifyStatusAction, ProtocolRunnerReadyAction,
    ProtocolRunnerResponseUnexpectedAction, ProtocolRunnerState,
};

pub fn protocol_runner_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::ProtocolRunnerStart(_) => {
            store.dispatch(ProtocolRunnerSpawnServerInitAction {});
        }
        Action::ProtocolRunnerSpawnServerSuccess(_) => {
            store.dispatch(ProtocolRunnerInitAction {});
        }
        Action::ProtocolRunnerInitSuccess(_) => {
            store.dispatch(ProtocolRunnerReadyAction {});
        }
        Action::ProtocolRunnerReady(_) => {
            match &store.state.get().protocol_runner {
                ProtocolRunnerState::Ready(state) => {
                    if let Some(hash) = state.genesis_commit_hash.as_ref() {
                        // Init storing of genesis block data, if genesis
                        // block wasn't committed before.
                        let genesis_commit_hash = hash.clone();
                        store.dispatch(StorageBlocksGenesisInitAction {
                            genesis_commit_hash,
                        });
                    }
                    store.dispatch(ProtocolRunnerNotifyStatusAction {});
                }
                _ => {}
            }
        }
        Action::ProtocolRunnerNotifyStatus(_) => {
            // TODO: notify failure too.
            store.service.protocol_runner().notify_status(true);
        }
        Action::WakeupEvent(_) => {
            while let Ok(result) = store.service.protocol_runner().try_recv() {
                let init_state = match &store.state().protocol_runner {
                    ProtocolRunnerState::SpawnServer(state) => match state {
                        ProtocolRunnerSpawnServerState::Pending {} => match result {
                            ProtocolRunnerResult::SpawnServer(Err(_)) => {
                                store.dispatch(ProtocolRunnerSpawnServerErrorAction {});
                                continue;
                            }
                            ProtocolRunnerResult::SpawnServer(Ok(_)) => {
                                store.dispatch(ProtocolRunnerSpawnServerSuccessAction {});
                                continue;
                            }
                            result => {
                                store.dispatch(ProtocolRunnerResponseUnexpectedAction { result });
                                continue;
                            }
                        },
                        _ => {
                            store.dispatch(ProtocolRunnerResponseUnexpectedAction { result });
                            continue;
                        }
                    },
                    ProtocolRunnerState::Init(v) => v,
                    ProtocolRunnerState::Idle => {
                        store.dispatch(ProtocolRunnerResponseUnexpectedAction { result });
                        continue;
                    }
                    ProtocolRunnerState::Ready(_) => {
                        store.dispatch(ProtocolRunnerResponseAction { result });
                        continue;
                    }
                };
                match init_state {
                    ProtocolRunnerInitState::Runtime(ProtocolRunnerInitRuntimeState::Pending {
                        ..
                    }) => match result {
                        ProtocolRunnerResult::InitRuntime((token, Err(_))) => {
                            store.dispatch(ProtocolRunnerInitRuntimeErrorAction { token });
                        }
                        ProtocolRunnerResult::InitRuntime((token, Ok(_))) => {
                            store.dispatch(ProtocolRunnerInitRuntimeSuccessAction { token });
                        }
                        result => {
                            store.dispatch(ProtocolRunnerResponseUnexpectedAction { result });
                        }
                    },
                    ProtocolRunnerInitState::Context(ProtocolRunnerInitContextState::Pending {
                        ..
                    }) => match result {
                        ProtocolRunnerResult::InitContext((token, Err(_))) => {
                            store.dispatch(ProtocolRunnerInitContextErrorAction {
                                token: Some(token),
                            });
                        }
                        ProtocolRunnerResult::InitContext((token, Ok(result))) => {
                            store
                                .dispatch(ProtocolRunnerInitContextSuccessAction { token, result });
                        }
                        result => {
                            store.dispatch(ProtocolRunnerResponseUnexpectedAction { result });
                        }
                    },
                    _ => {
                        store.dispatch(ProtocolRunnerResponseUnexpectedAction { result });
                    }
                }
            }
        }
        _ => {}
    }
}
