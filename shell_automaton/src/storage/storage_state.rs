use serde::{Deserialize, Serialize};

use crate::request::PendingRequests;
use crate::storage::request::StorageRequestState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageState {
    pub requests: PendingRequests<StorageRequestState>,
}

impl StorageState {
    pub fn new() -> Self {
        Self {
            requests: PendingRequests::new(),
        }
    }
}
