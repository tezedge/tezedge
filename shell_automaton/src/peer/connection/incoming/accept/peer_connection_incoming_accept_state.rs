// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::peer::PeerToken;
use crate::service::mio_service::PeerConnectionIncomingAcceptError;

use super::PeerConnectionIncomingRejectedReason;

#[cfg(fuzzing)]
use crate::fuzzing::net::SocketAddrMutator;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionIncomingAcceptState {
    Idle {
        time: u64,
    },
    Success {
        time: u64,
        token: PeerToken,
        #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
        address: SocketAddr,
    },
    Error {
        time: u64,
        error: PeerConnectionIncomingAcceptError,
    },
    Rejected {
        time: u64,
        token: PeerToken,
        #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
        address: SocketAddr,
        reason: PeerConnectionIncomingRejectedReason,
    },
}
