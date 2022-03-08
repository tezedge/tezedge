// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::service::storage_service::{
    StorageRequestPayload, StorageResponseError, StorageResponseSuccess,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageRequestStatus {
    Idle {
        time: u64,
    },
    Pending {
        time: u64,
    },
    Error {
        time: u64,
        pending_since: u64,
        error: StorageResponseError,
    },
    Success {
        time: u64,
        pending_since: u64,
        result: StorageResponseSuccess,
    },
}

impl StorageRequestStatus {
    pub fn pending_since(&self) -> Option<u64> {
        Some(match self {
            Self::Pending { time, .. } => *time,
            Self::Error { pending_since, .. } => *pending_since,
            Self::Success { pending_since, .. } => *pending_since,
            _ => return None,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageRequestor {
    // Internal requestors.
    None,
    Bootstrap,
    BlockApplier,

    // External requestors.
    Peer(SocketAddr),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestState {
    pub status: StorageRequestStatus,
    pub payload: StorageRequestPayload,
    pub requestor: StorageRequestor,
}
