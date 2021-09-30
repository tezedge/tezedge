#![allow(unused_imports)]
use bytes::Buf;
use redux_rs::{ActionWithId, Store};
use std::io::{Read, Write};
use tezos_messages::p2p::binary_message::CONTENT_LENGTH_FIELD_BYTES;

use crate::action::Action;
use crate::service::{MioService, Service};
use crate::storage::request::StorageRequestInitAction;
use crate::State;

use super::{
    StorageBlockHeaderPutNextInitAction, StorageBlockHeaderPutNextPendingAction,
    StorageBlockHeaderPutState, StorageBlockHeadersPutAction,
};

pub fn storage_block_header_put_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::StorageBlockHeadersPut(_) => {
            store.dispatch(StorageBlockHeaderPutNextInitAction {}.into());
        }
        Action::StorageBlockHeaderPutNextInit(_) => {
            match store.state.get().storage.block_headers_put.front() {
                Some(StorageBlockHeaderPutState::Init { req_id, .. }) => {
                    let req_id = req_id.clone();
                    store.dispatch(StorageBlockHeaderPutNextPendingAction { req_id }.into());
                }
                _ => {}
            }
        }
        Action::StorageBlockHeaderPutNextPending(action) => {
            store.dispatch(
                StorageRequestInitAction {
                    req_id: action.req_id.clone(),
                }
                .into(),
            );
            store.dispatch(StorageBlockHeaderPutNextInitAction {}.into());
        }
        _ => {}
    }
}
