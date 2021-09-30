use redux_rs::ActionWithId;

use crate::action::Action;
use crate::service::storage_service::StorageRequestPayload;
use crate::storage::request::{StorageRequestState, StorageRequestStatus};
use crate::State;

use super::StorageBlockHeaderPutState;

pub fn storage_block_header_put_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::StorageBlockHeadersPut(action) => {
            let iter = action
                .block_headers
                .iter()
                .cloned()
                .map(|block_header| StorageBlockHeaderPutState::Idle(block_header));
            state.storage.block_headers_put.extend(iter);
        }
        Action::StorageBlockHeaderPutNextInit(_) => {
            // allow only 2 concurrent pending storage requests.
            if state.storage.requests.len() < 2 {
                if let Some(put_state) = state.storage.block_headers_put.front_mut() {
                    let block_header = match put_state {
                        StorageBlockHeaderPutState::Idle(header) => header.clone(),
                        _ => return,
                    };

                    let req_id = state.storage.requests.add(StorageRequestState {
                        status: StorageRequestStatus::Idle,
                        payload: StorageRequestPayload::BlockHeaderWithHashPut(
                            block_header.clone(),
                        ),
                    });
                    *put_state = StorageBlockHeaderPutState::Init {
                        req_id,
                        block_header,
                    };
                }
            }
        }
        Action::StorageBlockHeaderPutNextPending(action) => {
            match state.storage.block_headers_put.front() {
                Some(StorageBlockHeaderPutState::Init { req_id, .. }) => {
                    if *req_id == action.req_id {
                        state.storage.block_headers_put.pop_front();
                    }
                }
                _ => return,
            };
        }
        _ => {}
    }
}
