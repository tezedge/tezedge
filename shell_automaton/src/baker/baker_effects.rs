// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::baker_service::{BakerService, BakerWorkerMessage};
use crate::{Action, ActionWithMeta, Service, Store};

use super::block_baker::{BakerBlockBakerComputeProofOfWorkSuccessAction, BakerBlockBakerState};

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
                }
            }
        }
        _ => {}
    }
}
