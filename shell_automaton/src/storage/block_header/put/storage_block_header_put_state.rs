use serde::{Deserialize, Serialize};

use storage::BlockHeaderWithHash;

use crate::request::RequestId;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageBlockHeaderPutState {
    Idle(BlockHeaderWithHash),
    Init {
        block_header: BlockHeaderWithHash,
        req_id: RequestId,
    },
}
