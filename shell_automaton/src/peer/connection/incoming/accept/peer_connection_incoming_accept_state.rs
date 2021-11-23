// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::peer::PeerToken;
use crate::service::mio_service::PeerConnectionIncomingAcceptError;

use super::PeerConnectionIncomingRejectedReason;

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::{PeerTokenMutator, SocketAddrMutator};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionIncomingAcceptState {
    Idle {
        time: u64,
    },
    Success {
        time: u64,
        #[cfg_attr(feature = "fuzzing", field_mutator(PeerTokenMutator))]
        token: PeerToken,
        #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
        address: SocketAddr,
    },
    Error {
        time: u64,
        error: PeerConnectionIncomingAcceptError,
    },
    Rejected {
        time: u64,
        #[cfg_attr(feature = "fuzzing", field_mutator(PeerTokenMutator))]
        token: PeerToken,
        #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
        address: SocketAddr,
        reason: PeerConnectionIncomingRejectedReason,
    },
}
