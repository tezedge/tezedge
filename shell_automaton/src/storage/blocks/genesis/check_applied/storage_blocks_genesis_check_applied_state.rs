// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use storage::block_meta_storage::Meta;

use crate::request::RequestId;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageBlocksGenesisCheckAppliedState {
    Idle,

    GetMetaInit,
    GetMetaPending { req_id: RequestId },
    GetMetaError {},
    GetMetaSuccess { meta: Option<Meta> },

    Success { is_applied: bool },
}
