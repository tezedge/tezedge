// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use crypto::{
    blake2b::Blake2bError,
    hash::{BlockHash, ChainId, FromBytesError, OperationHash, Signature},
};
use tezos_encoding::{binary_reader::BinaryReaderError, binary_writer::BinaryWriterError};
use tezos_messages::p2p::{
    binary_message::BinaryRead,
    encoding::{
        block_header::{BlockHeader, Level},
        operation::Operation,
    },
};

use crate::rights::{Delegate, EndorsingRights, EndorsingRightsError};

use super::EndorsementValidationError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct Key {
    pub operation: OperationHash,
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.operation.to_base58_check())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct PrecheckerState {
    pub operations: HashMap<Key, PrecheckerOperationState>,
    pub applied_blocks: HashMap<BlockHash, AppliedBlockCache>,
}

impl PrecheckerState {
    pub(super) fn non_terminals(&self) -> impl Iterator<Item = (&Key, &PrecheckerOperationState)> {
        self.operations
            .iter()
            .filter(|(_, state)| !state.is_terminal())
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum_macros::AsRefStr)]
pub enum PrecheckerOperationState {
    Init {
        operation: Operation,
        operation_binary_encoding: Vec<u8>,
    },
    PendingContentDecoding {
        operation: Operation,
        operation_binary_encoding: Vec<u8>,
    },
    DecodedContentReady {
        operation: Operation,
        operation_binary_encoding: Vec<u8>,
        operation_decoded_contents: OperationDecodedContents,
    },
    PendingBlockApplication {
        operation: Operation,
        operation_binary_encoding: Vec<u8>,
        operation_decoded_contents: OperationDecodedContents,
    },
    BlockApplied {
        operation: Operation,
        operation_binary_encoding: Vec<u8>,
        operation_decoded_contents: OperationDecodedContents,
    },
    PendingEndorsingRights {
        operation: Operation,
        operation_binary_encoding: Vec<u8>,
        operation_decoded_contents: OperationDecodedContents,
    },
    EndorsingRightsReady {
        operation: Operation,
        operation_binary_encoding: Vec<u8>,
        operation_decoded_contents: OperationDecodedContents,
        endorsing_rights: EndorsingRights,
    },
    PendingOperationPrechecking {
        operation: Operation,
        operation_binary_encoding: Vec<u8>,
        operation_decoded_contents: OperationDecodedContents,
        endorsing_rights: EndorsingRights,
    },
    Applied {
        protocol_data: String,
    },
    Refused {
        // TODO remove when all EC types are supported
        operation: Operation,
        protocol_data: String,
        error: EndorsementValidationError,
    },
    ProtocolNeeded,
    Error {
        operation: Option<Operation>,
        error: PrecheckerError,
    },
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

    pub(super) fn block_hash(&self) -> Option<&BlockHash> {
        self.operation().map(Operation::branch)
    }

    pub(super) fn operation(&self) -> Option<&Operation> {
        match self {
            PrecheckerOperationState::Init { operation, .. }
            | PrecheckerOperationState::PendingContentDecoding { operation, .. }
            | PrecheckerOperationState::DecodedContentReady { operation, .. }
            | PrecheckerOperationState::PendingBlockApplication { operation, .. }
            | PrecheckerOperationState::BlockApplied { operation, .. }
            | PrecheckerOperationState::PendingEndorsingRights { operation, .. }
            | PrecheckerOperationState::EndorsingRightsReady { operation, .. }
            | PrecheckerOperationState::PendingOperationPrechecking { operation, .. } => {
                Some(operation)
            }
            _ => None,
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
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppliedBlockCache {
    pub chain_id: ChainId,
    pub block_header: BlockHeader,
}
