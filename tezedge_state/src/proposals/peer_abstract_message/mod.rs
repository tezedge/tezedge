use crypto::CryptoError;
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::p2p::binary_message::BinaryChunk;
use tezos_messages::p2p::encoding::peer::PeerMessage;
use tezos_messages::p2p::encoding::prelude::{ConnectionMessage, MetadataMessage, AckMessage};

use crate::PeerCrypto;

mod peer_binary_message;
pub use peer_binary_message::*;

mod peer_decoded_message;
pub use peer_decoded_message::*;

#[derive(Debug)]
pub enum PeerAbstractMessageError {
    InvalidMessage,
    Crypto(CryptoError),
    Decode(BinaryReaderError),
}

impl From<BinaryReaderError> for PeerAbstractMessageError {
    fn from(err: BinaryReaderError) -> Self {
        Self::Decode(err)
    }
}

impl From<CryptoError> for PeerAbstractMessageError {
    fn from(err: CryptoError) -> Self {
        Self::Crypto(err)
    }
}

pub trait PeerAbstractMessage {
    fn take_binary_chunk(self) -> BinaryChunk;
    fn binary_chunk(&self) -> &BinaryChunk;
    fn as_connection_msg(&mut self) -> Result<ConnectionMessage, PeerAbstractMessageError>;
    fn as_metadata_msg(&mut self, crypto: &mut PeerCrypto) -> Result<MetadataMessage, PeerAbstractMessageError>;
    fn as_ack_msg(&mut self, crypto: &mut PeerCrypto) -> Result<AckMessage, PeerAbstractMessageError>;
    fn as_peer_msg(&mut self, crypto: &mut PeerCrypto) -> Result<PeerMessage, PeerAbstractMessageError>;
}