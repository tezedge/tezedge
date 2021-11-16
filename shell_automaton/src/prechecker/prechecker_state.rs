// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use crypto::hash::{BlockHash, OperationHash, Signature};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::p2p::encoding::{block_header::Level, operation::Operation};

use crate::rights::{Delegate, Slot};

 #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct Key {
    pub operation: OperationHash,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct PrecheckerState {
    pub operations: HashMap<Key, PrecheckerOperationState>
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
    Endorsement(Endorsement),
    EndorsementWithSlot(PrevalidatorEndorsementOperation),
    Other,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PrecheckerOperationState {
    Init { operation: Operation, },
    PendingContentDecoding { operation: Operation, },
    DecodedContentReady { operation_decoded_contents: OperationDecodedContents, },
    PendingEndorsingRights { operation_decoded_contents: OperationDecodedContents, },
    EndorsingRightsReady { operation_decoded_contents: OperationDecodedContents, },
    PendingOperationPrechecking { operation_decoded_contents: OperationDecodedContents, },
    Ready { verdict: Result<(), PrecheckerValidationError>, },
    Error { error: PrecheckerError },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PrecheckerError {
    Foo
    // TODO
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
