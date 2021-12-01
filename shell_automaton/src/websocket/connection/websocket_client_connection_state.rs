// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::peer::PeerToken;
use crate::service::mio_service::WebSocketIncomingAcceptError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WebSocketConnectionIncomingAcceptState {
    Idle {
        time: u64,
    },
    Success {
        time: u64,
        token: PeerToken,
    },
    Error {
        time: u64,
        error: WebSocketIncomingAcceptError,
    },
    // TODO: HandshakePaused?
    // TODO
    // Rejected {
    //     time: u64,
    //     token: PeerToken,
    //     address: SocketAddr,
    //     reason: ?,
    // },
}
