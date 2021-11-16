// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use serde::{Deserialize, Serialize};

use crate::peer::PeerToken;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerDisconnecting {
    pub token: PeerToken,
}

#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum PeerDisconnectionState {
    Disconnecting(PeerDisconnecting),
    Disconnected,
}
