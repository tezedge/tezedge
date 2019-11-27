// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use shell::shell_channel::BlockApplied;

use crate::helpers::FullBlockInfo;

/// Request/Response to access the Current Head data from RpcActor
#[derive(Debug, Clone)]
pub enum GetCurrentHead {
    Request,
    Response(Option<BlockApplied>),
}

/// Request/Response to access the Current Head data from RpcActor
#[derive(Debug, Clone)]
pub enum GetFullCurrentHead {
    Request,
    Response(Option<FullBlockInfo>),
}


