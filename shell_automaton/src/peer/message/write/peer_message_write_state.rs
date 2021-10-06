use crypto::nonce::Nonce;
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, sync::Arc};

use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use crate::peer::{
    binary_message::{read::PeerBinaryMessageReadState, write::PeerBinaryMessageWriteState},
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageWriteState {
    pub queue: VecDeque<Arc<PeerMessageResponse>>,
    pub current: PeerBinaryMessageWriteState,
}
