// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::service::storage_service::{
    StorageRequestPayload, StorageResponseError, StorageResponseSuccess,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageRequestStatus {
    Idle,
    Pending,
    Error(StorageResponseError),
    Success(StorageResponseSuccess),
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
