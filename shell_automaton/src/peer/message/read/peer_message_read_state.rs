use crypto::nonce::Nonce;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use crate::peer::{
    binary_message::read::PeerBinaryMessageReadState,
    chunk::read::{PeerChunkRead, PeerChunkReadError, ReadCrypto},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerBinaryMessageReadError {
    Chunk(PeerChunkReadError),
    Decode(String),
}

impl From<BinaryReaderError> for PeerBinaryMessageReadError {
    fn from(error: BinaryReaderError) -> Self {
        Self::Decode(error.to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, strum_macros::AsRefStr)]
pub enum PeerMessageReadState {
    Pending {
        binary_message_read: PeerBinaryMessageReadState,
    },
    Success {
        read_crypto: ReadCrypto,
        message: Arc<PeerMessageResponse>,
    },
}
