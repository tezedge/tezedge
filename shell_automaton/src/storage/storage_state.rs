use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::request::PendingRequests;
use crate::storage::block_header::put::StorageBlockHeaderPutState;
use crate::storage::request::StorageRequestState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageState {
    pub block_headers_put: VecDeque<StorageBlockHeaderPutState>,
    pub requests: PendingRequests<StorageRequestState>,
}

impl StorageState {
    pub fn new() -> Self {
        Self {
            block_headers_put: VecDeque::new(),
            requests: PendingRequests::new(),
        }
    }
}
