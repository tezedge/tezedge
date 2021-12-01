// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, convert::TryFrom};

use crypto::{
    base58::FromBase58CheckError,
    blake2b::Blake2bError,
    hash::{BlockHash, ChainId, FromBytesError, HashBase58, OperationHash, Signature},
};
use redux_rs::ActionId;
use tezos_encoding::{binary_reader::BinaryReaderError, binary_writer::BinaryWriterError};
use tezos_messages::p2p::{
    binary_message::BinaryRead,
    encoding::{
        block_header::{BlockHeader, Level},
        operation::Operation,
    },
};

use crate::rights::{Delegate, EndorsingRights, EndorsingRightsError};

use super::{EndorsementValidationError, OperationProtocolData};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
#[serde(into = "String", try_from = "String")]
pub struct Key {
    pub operation: OperationHash,
}

impl From<Key> for String {
    fn from(source: Key) -> Self {
        source.operation.to_base58_check()
    }
}

impl TryFrom<String> for Key {
    type Error = FromBase58CheckError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        OperationHash::from_base58_check(&value).map(|operation| Self { operation })
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.operation.to_base58_check())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct PrecheckerState {
    pub operations: HashMap<Key, PrecheckerOperation>,
    pub current_block: Option<CurrentBlock>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentBlock {
    pub block_hash: BlockHash,
    pub block_header: BlockHeader,
    pub chain_id: ChainId,
}

impl PrecheckerState {
    pub(super) fn non_terminals(&self) -> impl Iterator<Item = (&Key, &PrecheckerOperation)> {
        self.operations
            .iter()
            .filter(|(_, state)| !state.state.is_terminal())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrevalidatorEndorsementOperation {
    pub branch: BlockHash,
    pub signature: Signature,
    pub level: Level,
    pub signed_contents: Vec<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrevalidatorEndorsementWithSlotOperation {
    pub branch: BlockHash,
    pub signature: Signature,
    pub level: Level,
    pub signed_contents: Vec<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum OperationDecodedContents {
    Proto010(tezos_messages::protocol::proto_010::operation::Operation),
}

impl OperationDecodedContents {
    pub(super) fn endorsement_level(&self) -> Option<Level> {
        match self {
            OperationDecodedContents::Proto010(operation) => operation.endorsement_level(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerOperation {
    pub start: ActionId,
    pub operation: Operation,
    pub operation_binary_encoding: Vec<u8>,
    pub state: PrecheckerOperationState,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum_macros::AsRefStr)]
pub enum PrecheckerOperationState {
    Init,
    PendingContentDecoding,
    DecodedContentReady {
        operation_decoded_contents: OperationDecodedContents,
    },
    PendingBlockApplication {
        operation_decoded_contents: OperationDecodedContents,
        level: Level,
    },
    BlockApplied {
        operation_decoded_contents: OperationDecodedContents,
        level: Level,
    },
    PendingEndorsingRights {
        operation_decoded_contents: OperationDecodedContents,
    },
    EndorsingRightsReady {
        operation_decoded_contents: OperationDecodedContents,
        endorsing_rights: EndorsingRights,
    },
    PendingOperationPrechecking {
        operation_decoded_contents: OperationDecodedContents,
        endorsing_rights: EndorsingRights,
    },
    Applied {
        protocol_data: String,
    },
    Refused {
        protocol_data: String,
        error: EndorsementValidationError,
    },
    ProtocolNeeded,
    Error {
        error: PrecheckerError,
    },
}

impl PrecheckerOperation {
    pub(super) fn block_hash(&self) -> &BlockHash {
        self.operation.branch()
    }
}

impl PrecheckerOperationState {
    fn is_terminal(&self) -> bool {
        match self {
            Self::Applied { .. }
            | Self::Refused { .. }
            | Self::ProtocolNeeded
            | Self::Error { .. } => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum PrecheckerResponseError {
    #[error("Error converting to binary encoding: {0}")]
    Encoding(String),
    #[error("Error calculating hash: {0}")]
    Hashing(#[from] Blake2bError),
    #[error("Error converting to operation hash: {0}")]
    TypedHash(#[from] FromBytesError),
    #[error("Error getting endorsing righst: {0}")]
    Rights(#[from] EndorsingRightsError),
    #[error("Error parsing protocol data: {0}")]
    Decode(#[from] BinaryReaderError),
}

impl From<BinaryWriterError> for PrecheckerResponseError {
    fn from(error: BinaryWriterError) -> Self {
        Self::Encoding(error.to_string())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum PrecheckerError {
    #[error("Error decoding protocol specific operation contents: {0}")]
    OperationContentsDecode(#[from] BinaryReaderError),
    #[error("Error getting endorsing rights: {0}")]
    EndorsingRights(#[from] EndorsingRightsError),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum PrecheckerValidationError {
    #[error("Error parsing operation content: {0}")]
    DecodingError(#[from] BinaryReaderError),
    #[error("Delegate {0:?} does not have endorsing rights")]
    NoEndorsingRights(Delegate),
    #[error("Failed to verify the operation's signature")]
    SignatureError,
    #[error("Failed to verify the operation's inlined signature")]
    InlinedSignatureError,
}

impl OperationDecodedContents {
    pub(super) fn parse(encoded: &[u8]) -> Result<Self, BinaryReaderError> {
        let decoded =
            tezos_messages::protocol::proto_010::operation::Operation::from_bytes(encoded)?;
        Ok(Self::Proto010(decoded))
    }

    pub(super) fn is_endorsement(&self) -> bool {
        match self {
            OperationDecodedContents::Proto010(operation) if operation.contents.len() == 1 => match operation.contents[0] {
                tezos_messages::protocol::proto_010::operation::Contents::Endorsement(_) |
                tezos_messages::protocol::proto_010::operation::Contents::EndorsementWithSlot(_) => true,
                _ => false,
            }
            _ => false,
        }
    }

    pub(super) fn as_json(&self) -> serde_json::Value {
        match self {
            OperationDecodedContents::Proto010(operation) => operation.as_json(),
        }
    }
}

impl PrecheckerState {
    pub fn operations_for_block(
        &self,
        block_hash: Option<BlockHash>,
    ) -> HashMap<HashBase58<OperationHash>, PrecheckerOperationState> {
        let block_hash = if let Some(block_hash) = block_hash.as_ref() {
            block_hash
        } else if let Some(current) = &self.current_block {
            &current.block_hash
        } else {
            return HashMap::default();
        };
        self.operations
            .iter()
            .filter(|(_, op)| op.operation.branch() == block_hash)
            .map(|(key, op)| (key.operation.clone().into(), op.state.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crypto::hash::OperationHash;

    use super::Key;

    #[test]
    fn can_serialize_hash_map() {
        let hash_map = HashMap::from([(
            Key {
                operation: OperationHash::from_base58_check(
                    "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ",
                )
                .unwrap(),
            },
            true,
        )]);
        let json = serde_json::to_string(&hash_map).unwrap();
        let deserialized: HashMap<_, _> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, hash_map);
    }
}
