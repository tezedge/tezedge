// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, HashMap},
    convert::TryFrom,
    time::Duration,
};

use crypto::{
    base58::FromBase58CheckError,
    blake2b::Blake2bError,
    hash::{BlockHash, BlockPayloadHash, FromBytesError, OperationHash, ProtocolHash, Signature},
};
use redux_rs::ActionId;
use tezos_encoding::{binary_reader::BinaryReaderError, binary_writer::BinaryWriterError};
use tezos_messages::{
    p2p::{
        binary_message::BinaryRead,
        encoding::{
            block_header::{BlockHeader, Level},
            operation::Operation,
        },
    },
    protocol::{SupportedProtocol, UnsupportedProtocolError},
};

use crate::{
    rights::{Delegate, EndorsingRights, RightsError, Slot},
    storage::kv_block_additional_data::Error as BlockAdditionalDataStorageError,
};

use super::{EndorsementValidationError, OperationProtocolData};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
#[serde(into = "String", try_from = "String")]
pub struct Key {
    pub operation: OperationHash,
}

impl From<&OperationHash> for Key {
    fn from(op: &OperationHash) -> Self {
        Self {
            operation: op.clone(),
        }
    }
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
    pub blocks_cache: BTreeMap<BlockHash, (ActionId, BlockHeader)>,
    pub proto_cache: BTreeMap<u8, ProtocolHash>,
    pub protocol_cache: BTreeMap<BlockHash, (ActionId, ProtocolHash, ProtocolHash)>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SupportedProtocolState {
    None,
    Requesting(BlockHash),
    Ready(SupportedProtocol),
}

impl Default for SupportedProtocolState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtocolVersionCache {
    pub time: Duration,
    /// Mapping from block hash to next protocol, to be used to get protocol
    /// for incoming current head basing on its predecessor.
    pub next_protocol_versions: BTreeMap<BlockHash, (ActionId, SupportedProtocol)>,
}

impl Default for ProtocolVersionCache {
    fn default() -> ProtocolVersionCache {
        Self {
            time: Duration::from_secs(600),
            next_protocol_versions: Default::default(),
        }
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

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum OperationDecodedContents {
    Proto010(tezos_messages::protocol::proto_010::operation::Operation),
    Proto011(tezos_messages::protocol::proto_011::operation::Operation),
    Proto012(tezos_messages::protocol::proto_012::operation::Operation),
}

impl OperationDecodedContents {
    pub(super) fn endorsement_level(&self) -> Option<Level> {
        match self {
            OperationDecodedContents::Proto010(operation) => operation.endorsement_level(),
            OperationDecodedContents::Proto011(operation) => operation.endorsement_level(),
            OperationDecodedContents::Proto012(operation) => operation.endorsement_level(),
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
    PendingBlockPrechecked {
        operation_decoded_contents: OperationDecodedContents,
    },
    BlockPrecheckedReady {
        operation_decoded_contents: OperationDecodedContents,
    },
    PendingBlockApplied {
        operation_decoded_contents: OperationDecodedContents,
    },
    BlockAppliedReady {
        operation_decoded_contents: OperationDecodedContents,
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
        operation_decoded_contents: OperationDecodedContents,
    },
    Refused {
        operation_decoded_contents: OperationDecodedContents,
        error: EndorsementValidationError,
    },
    ProtocolNeeded,
    Error {
        error: PrecheckerError,
    },
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum PrecheckerResponseError {
    #[error("Error converting to binary encoding: {0}")]
    Encoding(String),
    #[error("Error calculating hash: {0}")]
    Hashing(#[from] Blake2bError),
    #[error("Error converting to operation hash: {0}")]
    TypedHash(#[from] FromBytesError),
    #[error("Error getting endorsing righst: {0}")]
    Rights(#[from] RightsError),
    #[error("Error parsing protocol data: {0}")]
    Decode(#[from] BinaryReaderError),
    #[error("Unknown protocol: {0}")]
    Protocol(#[from] UnsupportedProtocolError),
    #[error("Storage error: {0}")]
    Storage(#[from] BlockAdditionalDataStorageError),
    #[error("Error: `{0}`")]
    Other(#[from] PrecheckerError),
}

impl From<BinaryWriterError> for PrecheckerResponseError {
    fn from(error: BinaryWriterError) -> Self {
        Self::Encoding(error.to_string())
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum PrecheckerError {
    #[error("Missing block header for operation `{0}`")]
    MissingBlockHeader(BlockHash),
    #[error("Missing protocol for block `{0}`")]
    MissingProtocol(BlockHash),
    #[error("Unsupported protocol: `{0}`")]
    UnsupportedProtocol(#[from] UnsupportedProtocolError),
    #[error("Error decoding protocol specific operation contents: {0}")]
    OperationContentsDecode(#[from] BinaryReaderError),
    #[error("Error getting endorsing rights: {0}")]
    EndorsingRights(#[from] RightsError),
    #[error("Storage error: {0}")]
    Storage(#[from] BlockAdditionalDataStorageError),
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
    pub(super) fn parse(
        encoded: &[u8],
        proto: &SupportedProtocol,
    ) -> Result<Self, PrecheckerError> {
        Ok(match proto {
            SupportedProtocol::Proto010 => Self::Proto010(
                tezos_messages::protocol::proto_010::operation::Operation::from_bytes(encoded)?,
            ),
            SupportedProtocol::Proto011 => Self::Proto011(
                tezos_messages::protocol::proto_011::operation::Operation::from_bytes(encoded)?,
            ),
            SupportedProtocol::Proto012 => Self::Proto012(
                tezos_messages::protocol::proto_012::operation::Operation::from_bytes(encoded)?,
            ),
            _ => {
                return Err(PrecheckerError::UnsupportedProtocol(
                    UnsupportedProtocolError {
                        protocol: proto.protocol_hash(),
                    },
                ));
            }
        })
    }

    pub(crate) fn branch(&self) -> &BlockHash {
        match self {
            OperationDecodedContents::Proto010(op) => &op.branch,
            OperationDecodedContents::Proto011(op) => &op.branch,
            OperationDecodedContents::Proto012(op) => &op.branch,
        }
    }

    #[allow(unused)]
    pub(crate) fn payload(&self) -> Option<&BlockPayloadHash> {
        match self {
            OperationDecodedContents::Proto012(op) => op.payload(),
            _ => None,
        }
    }

    pub(crate) fn level_round(&self) -> Option<(i32, i32)> {
        match self {
            OperationDecodedContents::Proto012(op) => op.level_round(),
            _ => None,
        }
    }

    pub(crate) fn is_endorsement(&self) -> bool {
        match self {
            OperationDecodedContents::Proto010(operation) => operation.is_endorsement(),
            OperationDecodedContents::Proto011(operation) => operation.is_endorsement(),
            OperationDecodedContents::Proto012(operation) => operation.is_endorsement(),
        }
    }

    pub(crate) fn is_preendorsement(&self) -> bool {
        match self {
            OperationDecodedContents::Proto012(operation) => operation.is_preendorsement(),
            _ => false,
        }
    }

    pub(crate) fn endorsement_slot(&self) -> Option<Slot> {
        match self {
            OperationDecodedContents::Proto010(operation) => {
                operation.as_endorsement().map(|e| e.slot)
            }
            OperationDecodedContents::Proto011(operation) => {
                operation.as_endorsement().map(|e| e.slot)
            }
            OperationDecodedContents::Proto012(operation) => operation.slot(),
        }
    }

    pub(crate) fn as_json(&self) -> serde_json::Value {
        match self {
            OperationDecodedContents::Proto010(operation) => operation.as_json(),
            OperationDecodedContents::Proto011(operation) => operation.as_json(),
            OperationDecodedContents::Proto012(operation) => operation.as_json(),
        }
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
