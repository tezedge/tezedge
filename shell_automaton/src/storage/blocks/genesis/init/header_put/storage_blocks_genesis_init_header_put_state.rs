// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::request::RequestId;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageBlocksGenesisInitHeaderPutState {
    Init,
    Pending { req_id: RequestId },
    Error {},
    Success { is_new_block: bool },
}
