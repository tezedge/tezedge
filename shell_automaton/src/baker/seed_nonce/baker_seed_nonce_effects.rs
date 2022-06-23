// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::OperationHash;
use tezos_messages::p2p::binary_message::MessageHash;

use crate::mempool::MempoolOperationInjectAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    BakerSeedNonceCycleNextWaitAction, BakerSeedNonceFinishAction,
    BakerSeedNonceRevealIncludedAction, BakerSeedNonceRevealInitAction,
    BakerSeedNonceRevealMempoolInjectAction, BakerSeedNonceRevealPendingAction,
    BakerSeedNonceRevealSuccessAction, BakerSeedNonceState,
    SeedNonceRevelationOperationWithForgedBytes,
};

pub fn baker_seed_nonce_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::CurrentHeadUpdate(_) => {
            let bakers = store
                .state()
                .bakers
                .iter()
                .flat_map(|(baker, state)| {
                    state
                        .seed_nonces
                        .iter()
                        .map(|(level, _)| (baker.clone(), *level))
                })
                .collect::<Vec<_>>();

            for (baker, level) in bakers {
                store.dispatch(BakerSeedNonceRevealInitAction {
                    baker: baker.clone(),
                    level,
                });
                store.dispatch(BakerSeedNonceRevealIncludedAction {
                    baker: baker.clone(),
                    level,
                });
                store.dispatch(BakerSeedNonceRevealSuccessAction {
                    baker: baker.clone(),
                    level,
                });
                store.dispatch(BakerSeedNonceFinishAction {
                    baker: baker.clone(),
                    level,
                });
                store.dispatch(BakerSeedNonceRevealMempoolInjectAction {
                    baker: baker.clone(),
                    level,
                });
            }
        }
        Action::BakerSeedNonceCommitted(content) => {
            store.dispatch(BakerSeedNonceCycleNextWaitAction {
                baker: content.baker.clone(),
                level: content.level,
            });
        }
        Action::BakerSeedNonceRevealInit(content) => {
            let nonce = match store
                .state()
                .bakers
                .get(&content.baker)
                .and_then(|b| b.seed_nonces.get(&content.level))
            {
                Some(v) => match v {
                    BakerSeedNonceState::CycleNextWait { nonce, .. } => nonce.clone(),
                    _ => return,
                },
                None => return,
            };
            let operation = SeedNonceRevelationOperationWithForgedBytes::new(content.level, nonce);

            store.dispatch(BakerSeedNonceRevealPendingAction {
                baker: content.baker.clone(),
                level: content.level,
                operation,
            });
        }
        Action::BakerSeedNonceRevealPending(content) => {
            store.dispatch(BakerSeedNonceRevealMempoolInjectAction {
                baker: content.baker.clone(),
                level: content.level,
            });
        }
        Action::BakerSeedNonceRevealMempoolInject(content) => {
            let operation = store
                .state()
                .bakers
                .get(&content.baker)
                .and_then(|baker| baker.seed_nonces.get(&content.level))
                .and_then(|nonce_state| match nonce_state {
                    BakerSeedNonceState::RevealPending { operation, .. } => {
                        let branch = store.state().current_head.pred_hash()?;
                        Some(operation.as_p2p_operation(branch.clone()))
                    }
                    _ => None,
                });
            let operation = match operation {
                Some(v) => v,
                None => return,
            };
            let hash: OperationHash = operation.message_typed_hash().unwrap();
            slog::info!(store.state().log, "[BAKER] Injected SeedNonce Revelation operation.";
                "operation_hash" => hash.to_base58_check(),
                "branch" => operation.branch().to_base58_check());
            store.dispatch(MempoolOperationInjectAction {
                hash,
                operation,
                rpc_id: None,
                injected_timestamp: action.time_as_nanos(),
            });
        }
        Action::BakerSeedNonceRevealSuccess(content) => {
            store.dispatch(BakerSeedNonceFinishAction {
                baker: content.baker.clone(),
                level: content.level,
            });
        }
        _ => {}
    }
}
