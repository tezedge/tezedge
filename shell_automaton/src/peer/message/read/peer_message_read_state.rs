use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use crate::peer::binary_message::read::PeerBinaryMessageReadState;
use crate::peer::chunk::read::ReadCrypto;

#[derive(Error, Serialize, Deserialize, Debug, Clone)]
pub enum PeerMessageReadError {
    #[error("Error while decoding PeerMessage: {0}")]
    Decode(#[from] BinaryReaderError),
}

#[derive(Serialize, Deserialize, Debug, Clone, strum_macros::AsRefStr)]
pub enum PeerMessageReadState {
    Pending {
        binary_message_read: PeerBinaryMessageReadState,
    },
    Error {
        error: PeerMessageReadError,
    },
    Success {
        read_crypto: ReadCrypto,
        message: Arc<PeerMessageResponse>,
    },
}
