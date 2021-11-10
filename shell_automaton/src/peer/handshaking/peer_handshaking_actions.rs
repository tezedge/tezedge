// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use tezos_messages::p2p::{
    binary_message::BinaryChunk,
    encoding::{ack::AckMessage, connection::ConnectionMessage, metadata::MetadataMessage},
};

use crate::peer::PeerCrypto;
use crate::{EnablingCondition, State};

use super::PeerHandshakingError;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingInitAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerHandshakingInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingConnectionMessageInitAction {
    pub address: SocketAddr,
    pub message: ConnectionMessage,
}

impl EnablingCondition<State> for PeerHandshakingConnectionMessageInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingConnectionMessageEncodeAction {
    pub address: SocketAddr,
    pub binary_message: Vec<u8>,
}

impl EnablingCondition<State> for PeerHandshakingConnectionMessageEncodeAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingConnectionMessageWriteAction {
    pub address: SocketAddr,
    pub chunk: BinaryChunk,
}

impl EnablingCondition<State> for PeerHandshakingConnectionMessageWriteAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingConnectionMessageReadAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerHandshakingConnectionMessageReadAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingConnectionMessageDecodeAction {
    pub address: SocketAddr,
    pub message: ConnectionMessage,
    pub remote_chunk: BinaryChunk,
}

impl EnablingCondition<State> for PeerHandshakingConnectionMessageDecodeAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingEncryptionInitAction {
    pub address: SocketAddr,
    pub crypto: PeerCrypto,
}

impl EnablingCondition<State> for PeerHandshakingEncryptionInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingErrorAction {
    pub address: SocketAddr,
    pub error: PeerHandshakingError,
}

impl EnablingCondition<State> for PeerHandshakingErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

///////////////////////////////////////
///////////////////////////////////////

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingMetadataMessageInitAction {
    pub address: SocketAddr,
    pub message: MetadataMessage,
}

impl EnablingCondition<State> for PeerHandshakingMetadataMessageInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingMetadataMessageEncodeAction {
    pub address: SocketAddr,
    pub binary_message: Vec<u8>,
}

impl EnablingCondition<State> for PeerHandshakingMetadataMessageEncodeAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingMetadataMessageWriteAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerHandshakingMetadataMessageWriteAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingMetadataMessageReadAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerHandshakingMetadataMessageReadAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingMetadataMessageDecodeAction {
    pub address: SocketAddr,
    pub message: MetadataMessage,
}

impl EnablingCondition<State> for PeerHandshakingMetadataMessageDecodeAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

///////////////////////////////////////

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingAckMessageInitAction {
    pub address: SocketAddr,
    pub message: AckMessage,
}

impl EnablingCondition<State> for PeerHandshakingAckMessageInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingAckMessageEncodeAction {
    pub address: SocketAddr,
    pub binary_message: Vec<u8>,
}

impl EnablingCondition<State> for PeerHandshakingAckMessageEncodeAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingAckMessageWriteAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerHandshakingAckMessageWriteAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingAckMessageReadAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerHandshakingAckMessageReadAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingAckMessageDecodeAction {
    pub address: SocketAddr,
    pub message: AckMessage,
}

impl EnablingCondition<State> for PeerHandshakingAckMessageDecodeAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingFinishAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerHandshakingFinishAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
