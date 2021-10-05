use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use tezos_messages::p2p::{
    binary_message::BinaryChunk,
    encoding::{ack::AckMessage, connection::ConnectionMessage, metadata::MetadataMessage},
};

use crate::peer::PeerCrypto;

use super::PeerHandshakingError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingInitAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingConnectionMessageInitAction {
    pub address: SocketAddr,
    pub message: ConnectionMessage,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingConnectionMessageEncodeAction {
    pub address: SocketAddr,
    pub binary_message: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingConnectionMessageWriteAction {
    pub address: SocketAddr,
    pub chunk: BinaryChunk,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingConnectionMessageReadAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingConnectionMessageDecodeAction {
    pub address: SocketAddr,
    pub message: ConnectionMessage,
    pub remote_chunk: BinaryChunk,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingEncryptionInitAction {
    pub address: SocketAddr,
    pub crypto: PeerCrypto,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingErrorAction {
    pub address: SocketAddr,
    pub error: PeerHandshakingError,
}

///////////////////////////////////////
///////////////////////////////////////

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingMetadataMessageInitAction {
    pub address: SocketAddr,
    pub message: MetadataMessage,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingMetadataMessageEncodeAction {
    pub address: SocketAddr,
    pub binary_message: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingMetadataMessageWriteAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingMetadataMessageReadAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingMetadataMessageDecodeAction {
    pub address: SocketAddr,
    pub message: MetadataMessage,
}

///////////////////////////////////////

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingAckMessageInitAction {
    pub address: SocketAddr,
    pub message: AckMessage,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingAckMessageEncodeAction {
    pub address: SocketAddr,
    pub binary_message: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingAckMessageWriteAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingAckMessageReadAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingAckMessageDecodeAction {
    pub address: SocketAddr,
    pub message: AckMessage,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingFinishAction {
    pub address: SocketAddr,
}
