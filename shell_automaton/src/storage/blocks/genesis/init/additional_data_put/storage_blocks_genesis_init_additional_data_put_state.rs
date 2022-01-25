// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::request::RequestId;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageBlocksGenesisInitAdditionalDataPutState {
    Init {},
    Pending { req_id: RequestId },
    Error {},
    Success {},
}
