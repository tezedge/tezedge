// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;
use tezos_messages::p2p::encoding::block_header::BlockHeader;

use crate::request::RequestId;
use crate::service::storage_service::StorageError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerRemoteRequestsBlockHeaderGetCurrentState {
    Idle {
        time: u64,
    },
    Pending {
        block_hash: BlockHash,
        storage_req_id: RequestId,
    },
    Error {
        block_hash: BlockHash,
        error: StorageError,
    },
    Success {
        block_hash: BlockHash,
        result: Option<BlockHeader>,
    },
}

impl PeerRemoteRequestsBlockHeaderGetCurrentState {
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

    pub fn block_hash(&self) -> Option<&BlockHash> {
        match self {
            Self::Idle { .. } => None,
            Self::Pending { block_hash, .. }
            | Self::Error { block_hash, .. }
            | Self::Success { block_hash, .. } => Some(block_hash),
        }
    }
}

impl Default for PeerRemoteRequestsBlockHeaderGetCurrentState {
    fn default() -> Self {
        Self::Idle { time: 0 }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PeerRemoteRequestsBlockHeaderGetState {
    pub queue: BTreeSet<BlockHash>,
    pub current: PeerRemoteRequestsBlockHeaderGetCurrentState,
}
