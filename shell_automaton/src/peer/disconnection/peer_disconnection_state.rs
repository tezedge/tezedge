use derive_more::From;
use serde::{Deserialize, Serialize};

use crypto::crypto_box::PublicKey;
use tezos_messages::p2p::encoding::version::NetworkVersion;

use crate::peer::PeerToken;
use crate::Port;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerDisconnecting {
    pub token: PeerToken,
}

#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum PeerDisconnectionState {
    Disconnecting(PeerDisconnecting),
    Disconnected,
}
