// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithMeta, Store};

use crate::types::{LevelState, RoundState};

use super::{
    action::*,
    service::ServiceDefault,
    state::{Config, State},
};

pub fn effects(store: &mut Store<State, ServiceDefault, Action>, action: &ActionWithMeta<Action>) {
    slog::info!(store.service().logger, "{:#?}", action.action);

    match &action.action {
        Action::GetChainIdInit(GetChainIdInitAction {}) => {
            if let Err(error) = store.service().client.wait_bootstrapped() {
                store.dispatch(GetChainIdErrorAction { error });
            }
            match store.service().client.get_chain_id() {
                Ok(chain_id) => {
                    store.dispatch(GetChainIdSuccessAction { chain_id });
                }
                Err(error) => {
                    store.dispatch(GetChainIdErrorAction { error });
                }
            }
        }
        Action::GetChainIdSuccess(_) => {
            store.dispatch(GetConstantsInitAction {});
        }
        Action::GetConstantsInit(GetConstantsInitAction {}) => {
            match store.service().client.get_constants() {
                Ok(constants) => {
                    store.dispatch(GetConstantsSuccessAction { constants });
                }
                Err(error) => {
                    store.dispatch(GetConstantsErrorAction { error });
                }
            }
        }
        Action::GetConstantsSuccess(_) => {
            // the result is stream of heads,
            // they will be dispatched from event loop
            let delegate = store.service().crypto.public_key_hash().clone();
            store
                .service()
                .client
                .monitor_proposals(
                    delegate,
                    // it is first time we listening proposals,
                    // we know nothing, so no timeout required
                    i64::MAX,
                    Action::Timeout,
                    Action::NewProposal,
                )
                .unwrap();
        }
        Action::NewProposal(NewProposalAction { .. }) => {
            store.dispatch(TimeoutScheduleAction {});
            store.dispatch(InjectPreendorsementInitAction {});
            store.dispatch(InjectEndorsementInitAction {});
            store
                .service()
                .client
                .monitor_operations(i64::MAX, Action::Timeout, Action::NewOperationSeen)
                .unwrap();
        }
        // split in two, sign and inject
        Action::InjectPreendorsementInit(InjectPreendorsementInitAction {}) => {
            let Store { state, service, .. } = store;
            let (chain_id, preendorsement) = match state.get() {
                State::Ready {
                    config: Config { chain_id, .. },
                    preendorsement: Some(preendorsement),
                    ..
                } => (chain_id, preendorsement),
                _ => return,
            };
            slog::info!(service.logger, "{:#?}", preendorsement);
            let (data, _) = service.crypto.sign(0x12, chain_id, preendorsement).unwrap();
            let op = &hex::encode(data);
            service
                .client
                .inject_operation(chain_id, &op, i64::MAX, Action::Timeout, |hash| {
                    InjectPreendorsementSuccessAction { hash }.into()
                })
                .unwrap();
        }
        Action::InjectPreendorsementSuccess(InjectPreendorsementSuccessAction { .. }) => {}
        Action::TimeoutSchedule(TimeoutScheduleAction {}) => {
            match store.state() {
                &State::Ready {
                    round_state:
                        RoundState {
                            next_timeout: Some(timestamp),
                            ..
                        },
                    ..
                } => {
                    // slog::info!(
                    //     store.service().logger,
                    //     "timeout: {:?}",
                    //     chrono::TimeZone::timestamp(&chrono::Utc, timestamp.0 as i64, 0),
                    // );
                    store.service().timer.timeout(timestamp, Action::Timeout);
                }
                _ => (),
            }
        }
        Action::Timeout(TimeoutAction { now_timestamp }) => {
            store.dispatch(PreapplyBlockInitAction {
                timestamp: *now_timestamp,
            });
        }
        Action::NewOperationSeen(NewOperationSeenAction { .. }) => {}
        Action::InjectEndorsementInit(InjectEndorsementInitAction {}) => {
            let Store { state, service, .. } = store;
            let (chain_id, endorsement) = match state.get() {
                State::Ready {
                    config: Config { chain_id, .. },
                    endorsement: Some(endorsement),
                    ..
                } => (chain_id, endorsement),
                _ => return,
            };
            slog::info!(service.logger, "{:#?}", endorsement);
            let (data, _) = service.crypto.sign(0x13, chain_id, endorsement).unwrap();
            let op = &hex::encode(data);
            service
                .client
                .inject_operation(chain_id, &op, i64::MAX, Action::Timeout, |hash| {
                    InjectEndorsementSuccessAction { hash }.into()
                })
                .unwrap();
        }
        Action::InjectEndorsementSuccess(InjectEndorsementSuccessAction { .. }) => {}
        Action::PreapplyBlockInit(PreapplyBlockInitAction { timestamp }) => {
            let Store { state, service, .. } = store;
            let (chain_id, protocol_block_header, mempool) = match state.get() {
                State::Ready {
                    config: Config { chain_id, .. },
                    block: Some(v),
                    level_state: LevelState { mempool, .. },
                    ..
                } => (chain_id, v, mempool),
                _ => return,
            };
            slog::info!(service.logger, "{:#?}", protocol_block_header);
            let mut protocol_block_header = protocol_block_header.clone();
            protocol_block_header.signature.0.clear();
            let (_, signature) = service
                .crypto
                .sign(0x11, chain_id, &protocol_block_header)
                .unwrap();
            let mut protocol_block_header = protocol_block_header.clone();
            protocol_block_header.signature = signature;
            service
                .client
                .preapply_block(
                    protocol_block_header,
                    mempool.clone(),
                    *timestamp,
                    i64::MAX,
                    Action::Timeout,
                    |header, operations| PreapplyBlockSuccessAction { header, operations }.into(),
                )
                .unwrap();
        }
        Action::PreapplyBlockSuccess(PreapplyBlockSuccessAction { header, operations }) => {
            store.dispatch(InjectBlockInitAction {
                header: header.clone(),
                operations: operations.clone(),
            });
        }
        Action::InjectBlockInit(InjectBlockInitAction { header, operations }) => {
            let Store { state, service, .. } = store;
            let chain_id = match state.get() {
                State::Ready {
                    config: Config { chain_id, .. },
                    ..
                } => chain_id,
                _ => return,
            };
            slog::info!(service.logger, "{:#?}", header);
            let mut header = header.clone();
            header.signature.0.clear();
            let (data, _) = service.crypto.sign(0x11, chain_id, &header).unwrap();
            service
                .client
                .inject_block(
                    data,
                    operations.clone(),
                    i64::MAX,
                    Action::Timeout,
                    |hash| InjectBlockSuccessAction { hash }.into(),
                )
                .unwrap();
        }
        Action::InjectBlockSuccess(InjectBlockSuccessAction { .. }) => {}
        _ => {}
    }
}
