// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    convert::TryFrom,
};

use crypto::{
    blake2b::Blake2bError,
    hash::{BlockHash, BlockPayloadHash, FromBytesError, OperationHash},
};
use tezos_encoding::{binary_reader::BinaryReaderError, binary_writer::BinaryWriterError};
use tezos_messages::{
    p2p::encoding::{block_header::Level, operation::Operation},
    protocol::{
        proto_010, proto_011,
        proto_012::{self, operation::OperationVerifyError},
        proto_013, SupportedProtocol, UnsupportedProtocolError,
    },
};

use crate::rights::{Delegate, RightsError, Slot};

use super::{operation_contents::OperationDecodedContents, Round};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct EndorsementBranch {
    pub predecessor: BlockHash,
    pub level: Level,
    pub round: Round,
    pub payload_hash: BlockPayloadHash,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerState {
    pub endorsement_branch: Option<EndorsementBranch>,
    pub operations: HashMap<OperationHash, Result<PrecheckerOperation, PrecheckerError>>,
    pub cached_operations: CachedOperations,
}

impl PrecheckerState {
    pub(super) fn state(&self, hash: &OperationHash) -> Option<&PrecheckerOperationState> {
        let r = self.operations.get(hash)?;
        let op = r.as_ref().ok()?;
        Some(&op.state)
    }

    pub(crate) fn result(&self, hash: &OperationHash) -> Option<PrecheckerResult> {
        self.operations
            .get(hash)
            .and_then(|r| r.as_ref().ok())
            .and_then(|op| PrecheckerResult::try_from(op).ok())
    }

    pub(crate) fn operation(&self, hash: &OperationHash) -> Option<&Operation> {
        self.operations
            .get(hash)
            .and_then(|r| r.as_ref().ok())
            .map(PrecheckerOperation::operation)
    }
}

#[derive(Debug)]
pub(crate) struct PrecheckerResult<'a> {
    operation: &'a Operation,
    contents: Option<&'a OperationDecodedContents>,
    kind: PrecheckerResultKind<'a>,
}

#[derive(Debug)]
pub(crate) enum PrecheckerResultKind<'a> {
    Applied,
    Outdated,
    Refused(&'a PrecheckerError),
    BranchRefused,
    BranchDelayed,
}

impl<'a> PrecheckerResultKind<'a> {
    pub fn is_applied(&self) -> bool {
        matches!(self, Self::Applied)
    }
}

impl<'a> TryFrom<&'a PrecheckerOperation> for PrecheckerResult<'a> {
    type Error = ();

    fn try_from(source: &'a PrecheckerOperation) -> Result<Self, Self::Error> {
        let result = match &source.state {
            PrecheckerOperationState::Applied {
                operation_decoded_contents,
            } => PrecheckerResult {
                operation: &source.operation,
                contents: Some(operation_decoded_contents),
                kind: PrecheckerResultKind::Applied,
            },
            PrecheckerOperationState::Outdated {
                operation_decoded_contents,
            } => PrecheckerResult {
                operation: &source.operation,
                contents: Some(operation_decoded_contents),
                kind: PrecheckerResultKind::Outdated,
            },
            PrecheckerOperationState::Refused {
                operation_decoded_contents,
                error,
            } => PrecheckerResult {
                operation: &source.operation,
                contents: operation_decoded_contents.as_ref(),
                kind: PrecheckerResultKind::Refused(error),
            },
            PrecheckerOperationState::BranchDelayed {
                operation_decoded_contents,
                ..
            } => PrecheckerResult {
                operation: &source.operation,
                contents: Some(operation_decoded_contents),
                kind: PrecheckerResultKind::BranchDelayed,
            },
            PrecheckerOperationState::BranchRefused {
                operation_decoded_contents,
            } => PrecheckerResult {
                operation: &source.operation,
                contents: Some(operation_decoded_contents),
                kind: PrecheckerResultKind::BranchRefused,
            },
            _ => return Err(()),
        };
        Ok(result)
    }
}

impl<'a> PrecheckerResult<'a> {
    pub(crate) fn branch(&self) -> &BlockHash {
        self.operation.branch()
    }

    pub(crate) fn level(&self) -> Option<Level> {
        self.contents
            .and_then(OperationDecodedContents::level_round)
            .map(|(level, _)| level)
    }

    pub(crate) fn protocol_data(&self) -> serde_json::Value {
        self.contents
            .map_or(serde_json::Value::Null, |contents| contents.as_json())
    }

    pub(crate) fn error(&self) -> Option<String> {
        match &self.kind {
            PrecheckerResultKind::Applied => None,
            PrecheckerResultKind::Outdated => Some("operation is too old".into()),
            PrecheckerResultKind::Refused(error) => Some(error.to_string()),
            PrecheckerResultKind::BranchRefused => {
                Some("operation does not match the branch".into())
            }
            PrecheckerResultKind::BranchDelayed => {
                Some("operation does not match current branch".into())
            }
        }
    }

    pub(crate) fn protocol_as_str(&self) -> &'static str {
        match self.contents {
            Some(OperationDecodedContents::Proto010(_)) => proto_010::PROTOCOL_HASH,
            Some(OperationDecodedContents::Proto011(_)) => proto_011::PROTOCOL_HASH,
            Some(OperationDecodedContents::Proto012(_)) => proto_012::PROTOCOL_HASH,
            Some(OperationDecodedContents::Proto013(_)) => proto_013::PROTOCOL_HASH,
            None => "<no protocol>",
        }
    }

    pub(crate) fn operation(&self) -> &'a Operation {
        self.operation
    }

    pub(crate) fn contents(&self) -> Option<&'a OperationDecodedContents> {
        self.contents
    }

    pub(crate) fn kind(&self) -> &PrecheckerResultKind {
        &self.kind
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerOperation {
    pub operation: Operation,
    pub state: PrecheckerOperationState,
}

impl PrecheckerOperation {
    pub(super) fn new(operation: Operation, protocol: SupportedProtocol) -> Self {
        Self {
            operation,
            state: PrecheckerOperationState::Init { protocol },
        }
    }

    fn operation(&self) -> &Operation {
        &self.operation
    }

    pub(super) fn level(&self) -> Option<Level> {
        self.state
            .contents()
            .and_then(OperationDecodedContents::level_round)
            .map(|(level, _)| level)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TenderbakeConsensusContents {
    pub level: Level,
    pub round: Round,
    pub payload_hash: BlockPayloadHash,
    pub slot: Slot,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum_macros::AsRefStr)]
pub enum PrecheckerOperationState {
    Init {
        protocol: SupportedProtocol,
    },
    Decoded {
        operation_decoded_contents: OperationDecodedContents,
    },
    TenderbakeConsensus {
        operation_decoded_contents: OperationDecodedContents,
        consensus_contents: TenderbakeConsensusContents,
        endorsing_rights_verified: bool,
    },
    TenderbakePendingRights {
        operation_decoded_contents: OperationDecodedContents,
        consensus_contents: TenderbakeConsensusContents,
        endorsing_rights_verified: bool,
    },
    Applied {
        operation_decoded_contents: OperationDecodedContents,
    },
    Refused {
        operation_decoded_contents: Option<OperationDecodedContents>,
        error: PrecheckerError,
    },
    BranchRefused {
        operation_decoded_contents: OperationDecodedContents,
    },
    BranchDelayed {
        operation_decoded_contents: OperationDecodedContents,
        endorsing_rights_verified: bool,
    },
    Outdated {
        operation_decoded_contents: OperationDecodedContents,
    },
    ProtocolNeeded,
}

impl PrecheckerOperationState {
    pub(super) fn is_result(&self) -> bool {
        matches!(
            self,
            PrecheckerOperationState::Applied { .. }
                | PrecheckerOperationState::Refused { .. }
                | PrecheckerOperationState::BranchRefused { .. }
                | PrecheckerOperationState::BranchDelayed { .. }
                | PrecheckerOperationState::Outdated { .. }
        )
    }

    pub(super) fn caching_level(&self) -> Option<Level> {
        if let PrecheckerOperationState::BranchDelayed {
            operation_decoded_contents,
            ..
        } = self
        {
            operation_decoded_contents
                .level_round()
                .map(|(level, _)| level)
        } else {
            None
        }
    }

    pub(super) fn contents(&self) -> Option<&OperationDecodedContents> {
        match self {
            PrecheckerOperationState::Decoded {
                operation_decoded_contents,
            }
            | PrecheckerOperationState::TenderbakeConsensus {
                operation_decoded_contents,
                ..
            }
            | PrecheckerOperationState::Applied {
                operation_decoded_contents,
            }
            | PrecheckerOperationState::BranchRefused {
                operation_decoded_contents,
            }
            | PrecheckerOperationState::BranchDelayed {
                operation_decoded_contents,
                ..
            }
            | PrecheckerOperationState::Outdated {
                operation_decoded_contents,
            } => Some(operation_decoded_contents),
            PrecheckerOperationState::Refused {
                operation_decoded_contents,
                ..
            } => operation_decoded_contents.as_ref(),
            _ => None,
        }
    }
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
    // #[error("Storage error: {0}")]
    // Storage(#[from] BlockAdditionalDataStorageError),
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
    #[error("Error decoding protocol specific operation contents: `{0}`")]
    OperationContentsDecode(#[from] BinaryReaderError),
    #[error("Unsupported protocol: `{0}`")]
    UnsupportedProtocol(#[from] UnsupportedProtocolError),
    #[error("Error getting endorsing rights: `{0}`")]
    EndorsingRights(#[from] RightsError),
    #[error("Signature does not match")]
    SignatureVerificationError,
    #[error(transparent)]
    Consensus(#[from] ConsensusOperationError),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum ConsensusOperationError {
    #[error("Incorrect slot: `{0}`")]
    IncorrectSlot(Slot),
    #[error("Signature verification error: `{0}`")]
    SignatureVerificationError(String),
    #[error("Signature does not match")]
    SignatureMismatch,
    #[error("Wrong branch for consensus operation")]
    WrongBranch,
    #[error("Consensus operation for competing proposal")]
    CompetingProposal,
}

impl From<OperationVerifyError> for ConsensusOperationError {
    fn from(error: OperationVerifyError) -> Self {
        Self::SignatureVerificationError(error.to_string())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum PrecheckerValidationError {
    #[error("Error parsing operation content: {0}")]
    DecodingError(#[from] BinaryReaderError),
    #[error("Delegate `{0}` does not have endorsing rights")]
    NoEndorsingRights(Delegate),
    #[error("Failed to verify the operation's signature")]
    SignatureError,
    #[error("Failed to verify the operation's inlined signature")]
    InlinedSignatureError,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CachedOperations(BTreeMap<Level, BTreeSet<OperationHash>>);

impl CachedOperations {
    pub(super) fn insert(&mut self, level: Level, operation: OperationHash) {
        self.0.entry(level).or_default().insert(operation);
    }

    pub(super) fn remove_older(&mut self, level: Level) -> Vec<OperationHash> {
        let to_remove = self
            .0
            .keys()
            .filter_map(|l| if *l < level { Some(*l) } else { None })
            .collect::<Vec<_>>();
        let mut result: Vec<OperationHash> = Vec::new();
        for l in to_remove {
            if let Some(removed) = self.0.remove(&l) {
                result.extend(removed.into_iter());
            }
        }
        result
    }
}
