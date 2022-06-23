// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::baker_service::{BakerService, BakerWorkerMessage};
use crate::{Action, ActionWithMeta, Service, Store};

use super::block_baker::{
    BakerBlockBakerComputeProofOfWorkSuccessAction, BakerBlockBakerSignSuccessAction,
    BakerBlockBakerState,
};
use super::block_endorser::{
    BakerBlockEndorserEndorsementSignSuccessAction,
    BakerBlockEndorserPreendorsementSignSuccessAction, BakerBlockEndorserState,
};
use super::persisted::persist::{BakerPersistedPersistState, BakerPersistedPersistSuccessAction};
use super::persisted::rehydrate::{
    BakerPersistedRehydrateState, BakerPersistedRehydrateSuccessAction,
};

pub fn baker_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::WakeupEvent(_) => {
            while let Ok((result_req_id, result)) = store.service.baker().try_recv() {
                let mut bakers_iter = store.state().bakers.iter();
                match result {
                    BakerWorkerMessage::ComputeProofOfWork(nonce) => {
                        let baker = bakers_iter
                            .find(|(_, v)| match &v.block_baker {
                                BakerBlockBakerState::ComputeProofOfWorkPending {
                                    req_id, ..
                                } => result_req_id == *req_id,
                                _ => false,
                            })
                            .map(|(baker, _)| baker.clone());
                        let baker = match baker {
                            Some(v) => v,
                            None => continue,
                        };

                        store.dispatch(BakerBlockBakerComputeProofOfWorkSuccessAction {
                            baker,
                            proof_of_work_nonce: nonce.clone(),
                        });
                    }
                    BakerWorkerMessage::PreendorsementSign(result) => {
                        let baker = bakers_iter
                            .find(|(_, v)| match &v.block_endorser {
                                BakerBlockEndorserState::PreendorsementSignPending {
                                    req_id,
                                    ..
                                } => result_req_id == *req_id,
                                _ => false,
                            })
                            .map(|(baker, _)| baker.clone());
                        let baker = match baker {
                            Some(v) => v,
                            None => continue,
                        };
                        match result {
                            Ok(signature) => {
                                store.dispatch(BakerBlockEndorserPreendorsementSignSuccessAction {
                                    baker,
                                    signature,
                                });
                            }
                            Err(err) => todo!("PreendorsementSignError: {:?}", err),
                        }
                    }
                    BakerWorkerMessage::EndorsementSign(result) => {
                        let baker = bakers_iter
                            .find(|(_, v)| match &v.block_endorser {
                                BakerBlockEndorserState::EndorsementSignPending {
                                    req_id, ..
                                } => result_req_id == *req_id,
                                _ => false,
                            })
                            .map(|(baker, _)| baker.clone());
                        let baker = match baker {
                            Some(v) => v,
                            None => continue,
                        };
                        match result {
                            Ok(signature) => {
                                store.dispatch(BakerBlockEndorserEndorsementSignSuccessAction {
                                    baker,
                                    signature,
                                });
                            }
                            Err(err) => todo!("EndorsementSignError: {:?}", err),
                        }
                    }
                    BakerWorkerMessage::BlockSign(result) => {
                        let baker = bakers_iter
                            .find(|(_, v)| match &v.block_baker {
                                BakerBlockBakerState::SignPending { req_id, .. } => {
                                    result_req_id == *req_id
                                }
                                _ => false,
                            })
                            .map(|(baker, _)| baker.clone());
                        let baker = match baker {
                            Some(v) => v,
                            None => continue,
                        };
                        match result {
                            Ok(signature) => {
                                store.dispatch(BakerBlockBakerSignSuccessAction {
                                    baker,
                                    signature,
                                });
                            }
                            Err(err) => todo!("BlockSignError: {:?}", err),
                        }
                    }
                    BakerWorkerMessage::StateRehydrate(result) => {
                        let baker = bakers_iter
                            .find(|(_, v)| match &v.persisted.rehydrate {
                                BakerPersistedRehydrateState::Pending { req_id, .. } => {
                                    result_req_id == *req_id
                                }
                                _ => false,
                            })
                            .map(|(baker, _)| baker.clone());
                        let baker = match baker {
                            Some(v) => v,
                            None => continue,
                        };
                        match result {
                            Ok(result) => {
                                store.dispatch(BakerPersistedRehydrateSuccessAction {
                                    baker,
                                    result,
                                });
                            }
                            Err(err) => todo!("StateRehydrateError: {:?}", err),
                        }
                    }
                    BakerWorkerMessage::StatePersist(result) => {
                        let baker = bakers_iter
                            .find(|(_, v)| match &v.persisted.persist {
                                BakerPersistedPersistState::Pending { req_id, .. } => {
                                    result_req_id == *req_id
                                }
                                _ => false,
                            })
                            .map(|(baker, _)| baker.clone());
                        let baker = match baker {
                            Some(v) => v,
                            None => continue,
                        };
                        match result {
                            Ok(_) => {
                                store.dispatch(BakerPersistedPersistSuccessAction { baker });
                            }
                            Err(err) => todo!("StatePersistError: {:?}", err),
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
