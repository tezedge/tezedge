use serde::{Deserialize, Serialize};

use storage::BlockHeaderWithHash;

use crate::request::RequestId;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlockHeadersPutAction {
    pub block_headers: Vec<BlockHeaderWithHash>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlockHeaderPutNextInitAction;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlockHeaderPutNextPendingAction {
    pub req_id: RequestId,
}
