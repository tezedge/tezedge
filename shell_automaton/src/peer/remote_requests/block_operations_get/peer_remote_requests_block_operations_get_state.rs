// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use storage::OperationKey;
use tezos_messages::p2p::encoding::operations_for_blocks::OperationsForBlocksMessage;

use crate::request::RequestId;
use crate::service::storage_service::StorageError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerRemoteRequestsBlockOperationsGetCurrentState {
    Idle {
        time: u64,
    },
    Pending {
        key: OperationKey,
        storage_req_id: RequestId,
    },
    Error {
        key: OperationKey,
        error: StorageError,
    },
    Success {
        key: OperationKey,
        result: Option<OperationsForBlocksMessage>,
    },
}

impl PeerRemoteRequestsBlockOperationsGetCurrentState {
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    pub fn storage_req_id(&self) -> Option<RequestId> {
        match self {
            Self::Pending { storage_req_id, .. } => Some(*storage_req_id),
            _ => None,
        }
    }

    pub fn key(&self) -> Option<&OperationKey> {
        match self {
            Self::Idle { .. } => None,
            Self::Pending { key, .. } | Self::Error { key, .. } | Self::Success { key, .. } => {
                Some(key)
            }
        }
    }
}

impl Default for PeerRemoteRequestsBlockOperationsGetCurrentState {
    fn default() -> Self {
        Self::Idle { time: 0 }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PeerRemoteRequestsBlockOperationsGetState {
    pub queue: BTreeSet<OperationKey>,
    pub current: PeerRemoteRequestsBlockOperationsGetCurrentState,
}
