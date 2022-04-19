// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::current_head::CurrentHeadRehydrateInitAction;
use crate::protocol_runner::current_head::{
    ProtocolRunnerCurrentHeadInitAction, ProtocolRunnerCurrentHeadState,
    ProtocolRunnerCurrentHeadSuccessAction,
};
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

use super::init::context_ipc_server::{
    ProtocolRunnerInitContextIpcServerErrorAction, ProtocolRunnerInitContextIpcServerState,
    ProtocolRunnerInitContextIpcServerSuccessAction,
};
use super::init::ProtocolRunnerInitAction;
use super::spawn_server::ProtocolRunnerSpawnServerInitAction;
use super::{
    ProtocolRunnerNotifyStatusAction, ProtocolRunnerResponseUnexpectedAction,
    ProtocolRunnerShutdownPendingAction, ProtocolRunnerShutdownSuccessAction, ProtocolRunnerState,
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
            store.dispatch(ProtocolRunnerCurrentHeadInitAction {});
        }
        Action::ProtocolRunnerReady(_) => {
            if let ProtocolRunnerState::Ready(state) = &store.state.get().protocol_runner {
                if let Some(hash) = state.genesis_commit_hash.as_ref() {
                    // Init storing of genesis block data, if genesis
                    // block wasn't committed before.
                    let genesis_commit_hash = hash.clone();
                    store.dispatch(StorageBlocksGenesisInitAction {
                        genesis_commit_hash,
                    });
                }

                store.dispatch(CurrentHeadRehydrateInitAction {});
                store.dispatch(ProtocolRunnerNotifyStatusAction {});
            }
        }
        Action::ProtocolRunnerNotifyStatus(_) => {
            // TODO: notify failure too.
            store.service.protocol_runner().notify_status(true);
        }
        Action::ProtocolRunnerShutdownInit(_) => {
            store.service.protocol_runner().notify_status(false);
            match &store.state().protocol_runner {
                ProtocolRunnerState::Idle | ProtocolRunnerState::SpawnServer(_) => {
                    store.dispatch(ProtocolRunnerShutdownSuccessAction {});
                    return;
                }
                ProtocolRunnerState::ShutdownPending => return,
                _ => {}
            };

            store.service.protocol_runner().shutdown();
            store.dispatch(ProtocolRunnerShutdownPendingAction {});
        }
        Action::WakeupEvent(_) => {
            while let Ok(result) = store.service.protocol_runner().try_recv() {
                // TODO: clean this up
                let init_state = match &store.state().protocol_runner {
                    ProtocolRunnerState::SpawnServer(state) => match state {
                        ProtocolRunnerSpawnServerState::Pending {} => match result {
                            ProtocolRunnerResult::SpawnServer(Err(error)) => {
                                store.dispatch(ProtocolRunnerSpawnServerErrorAction { error });
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
                    ProtocolRunnerState::GetCurrentHead(state) => match state {
                        ProtocolRunnerCurrentHeadState::Pending { .. } => match result {
                            ProtocolRunnerResult::GetCurrentHead((
                                token,
                                Ok(latest_context_hashes),
                            )) => {
                                store.dispatch(ProtocolRunnerCurrentHeadSuccessAction {
                                    token,
                                    latest_context_hashes,
                                });
                                continue;
                            }
                            ProtocolRunnerResult::GetCurrentHead((token, Err(error))) => {
                                slog::error!(&store.state().log, "failed to get context's latest commits";
                                    "error" => format!("{:?}", error));
                                store.dispatch(ProtocolRunnerCurrentHeadSuccessAction {
                                    token,
                                    latest_context_hashes: Vec::new(),
                                });
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
                    ProtocolRunnerState::ShutdownPending => {
                        if let ProtocolRunnerResult::ShutdownServer(result) = result {
                            if let Err(err) = result {
                                slog::warn!(&store.state().log, "protocol runner shutdown failed";
                                    "error" => format!("{:?}", err));
                            }
                            store.dispatch(ProtocolRunnerShutdownSuccessAction {});
                        } else {
                            store.dispatch(ProtocolRunnerResponseAction { result });
                        }
                        continue;
                    }
                    ProtocolRunnerState::ShutdownSuccess => {
                        slog::warn!(&store.state().log, "message received from protocol runner after shutdown";
                            "message" => format!("{:?}", result));
                        continue;
                    }
                };

                match init_state {
                    ProtocolRunnerInitState::Runtime(ProtocolRunnerInitRuntimeState::Pending {
                        ..
                    }) => match result {
                        ProtocolRunnerResult::InitRuntime((token, Err(error))) => {
                            store.dispatch(ProtocolRunnerInitRuntimeErrorAction { token, error });
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
                        ProtocolRunnerResult::InitContext((token, Err(error))) => {
                            store.dispatch(ProtocolRunnerInitContextErrorAction {
                                token: Some(token),
                                error,
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
                    ProtocolRunnerInitState::ContextIpcServer((
                        _,
                        ProtocolRunnerInitContextIpcServerState::Pending { .. },
                    )) => match result {
                        ProtocolRunnerResult::InitContextIpcServer((token, Err(error))) => {
                            store.dispatch(ProtocolRunnerInitContextIpcServerErrorAction {
                                token,
                                error,
                            });
                        }
                        ProtocolRunnerResult::InitContextIpcServer((token, Ok(_))) => {
                            store.dispatch(ProtocolRunnerInitContextIpcServerSuccessAction {
                                token: Some(token),
                            });
                        }
                        result => {
                            store.dispatch(ProtocolRunnerResponseUnexpectedAction { result });
                        }
                    },
                    // TODO: be explicit, don't use wildcard matching
                    _ => {
                        store.dispatch(ProtocolRunnerResponseUnexpectedAction { result });
                    }
                }
            }
        }
        _ => {}
    }
}
