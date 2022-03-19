// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Instant;

use crypto::{
    hash::{BlockHash, ChainId},
    CryptoError,
};
use slog::{debug, FnValue, Logger};
use tezos_messages::{
    base::signature_public_key::SignatureWatermark,
    p2p::{binary_message::BinaryWrite, encoding::block_header::Level},
};

use crate::rights::{Delegate, EndorsingRights};

use super::OperationDecodedContents;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum EndorsementValidationError {
    #[error("Invalid endorsement wrapper")]
    InvalidEndorsementWrapper,
    #[error("Wrong endorsement predecessor")]
    WrongEndorsementPredecessor,
    #[error("Invalid endorsement level")]
    InvalidLevel,
    #[error("Invalid endorsement branch")]
    InvalidBranch,
    #[error("Non-endorement operation")]
    InvalidContents,
    #[error("Unwrapped endorsement is not supported")]
    UnwrappedEndorsement,
    #[error("Delegate {0:?} does not have endorsing rights")]
    NoEndorsingRights(Delegate),
    #[error("Error verifying operation signature")]
    SignatureError,
    #[error("Failed to verify the operation's signature")]
    SignatureMismatch,
    #[error("Non-existing slot")]
    InvalidSlot,
    #[error("Public key not found")]
    PublicKeyNotFound,
    #[error("Public key is not supported")]
    UnsupportedPublicKey,
    #[error("Binary encoding error")]
    EncodingError,
    #[error("Hash calculation error")]
    HashError,
    #[error("Failed to verify the operation's inlined signature")]
    InlinedSignatureMismatch,
    #[error("Cannot serialize as JSON")]
    JsonSerialization(String),
}

impl From<serde_json::Error> for EndorsementValidationError {
    fn from(error: serde_json::Error) -> Self {
        Self::JsonSerialization(error.to_string())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Applied {
    pub decoded_contents: OperationDecodedContents,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Refused {
    pub decoded_contents: OperationDecodedContents,
    pub error: EndorsementValidationError,
}

pub(super) trait OperationProtocolData {
    fn endorsement_level(&self) -> Option<Level>;
    fn as_json(&self) -> serde_json::Value;
}

impl OperationProtocolData for tezos_messages::protocol::proto_010::operation::Operation {
    fn endorsement_level(&self) -> Option<Level> {
        use tezos_messages::protocol::proto_010::operation::*;
        if self.contents.len() != 1 {
            return None;
        }
        match &self.contents[0] {
            Contents::Endorsement(EndorsementOperation { level }) => Some(*level),
            Contents::EndorsementWithSlot(EndorsementWithSlotOperation { endorsement, .. }) => {
                match endorsement.operations {
                    InlinedEndorsementContents::Endorsement(InlinedEndorsementVariant {
                        level,
                    }) => Some(level),
                }
            }
            _ => None,
        }
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!("cannot convert to json"))
    }
}

impl OperationProtocolData for tezos_messages::protocol::proto_011::operation::Operation {
    fn endorsement_level(&self) -> Option<Level> {
        use tezos_messages::protocol::proto_011::operation::*;
        if self.contents.len() != 1 {
            return None;
        }
        match &self.contents[0] {
            Contents::Endorsement(EndorsementOperation { level }) => Some(*level),
            Contents::EndorsementWithSlot(EndorsementWithSlotOperation { endorsement, .. }) => {
                match endorsement.operations {
                    InlinedEndorsementContents::Endorsement(InlinedEndorsementVariant {
                        level,
                    }) => Some(level),
                }
            }
            _ => None,
        }
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!("cannot convert to json"))
    }
}

pub(super) trait EndorsementValidator {
    fn validate_endorsement(
        &self,
        chain_id: &ChainId,
        block_hash: &BlockHash,
        rights: &EndorsingRights,
        log: &Logger,
    ) -> Result<Applied, Refused>;
}

impl EndorsementValidator for OperationDecodedContents {
    fn validate_endorsement(
        &self,
        chain_id: &ChainId,
        block_hash: &BlockHash,
        rights: &EndorsingRights,
        log: &Logger,
    ) -> Result<Applied, Refused> {
        let result = match self {
            OperationDecodedContents::Proto010(operation) => {
                validate_endorsement_010_granada(operation, chain_id, block_hash, rights, log)
            }
            OperationDecodedContents::Proto011(operation) => {
                validate_endorsement_011_hangzhou(operation, chain_id, block_hash, rights, log)
            }
        };
        match result {
            Ok(_) => Ok(Applied {
                decoded_contents: self.clone(),
            }),
            Err(error) => Err(Refused {
                decoded_contents: self.clone(),
                error,
            }),
        }
    }
}

fn validate_endorsement_010_granada(
    operation: &tezos_messages::protocol::proto_010::operation::Operation,
    chain_id: &ChainId,
    block_hash: &BlockHash,
    rights: &EndorsingRights,
    log: &Logger,
) -> Result<(), EndorsementValidationError> {
    use tezos_messages::protocol::proto_010::operation::*;

    let start = Instant::now();

    let contents = if operation.contents.len() == 1 {
        &operation.contents[0]
    } else {
        return Err(EndorsementValidationError::InvalidContents);
    };

    match contents {
        Contents::Endorsement(EndorsementOperation { level: _ }) => {
            Err(EndorsementValidationError::UnwrappedEndorsement)
        }
        Contents::EndorsementWithSlot(EndorsementWithSlotOperation { endorsement, slot }) => {
            if operation.signature.as_ref().iter().any(|b| *b != 0)
                || operation.branch != endorsement.branch
            {
                return Err(EndorsementValidationError::InvalidEndorsementWrapper);
            }

            validate_inlined_endorsement(
                endorsement,
                block_hash,
                *slot,
                rights,
                chain_id,
                start,
                log,
            )
        }
        _ => Err(EndorsementValidationError::InvalidContents),
    }
}

fn validate_endorsement_011_hangzhou(
    operation: &tezos_messages::protocol::proto_011::operation::Operation,
    chain_id: &ChainId,
    block_hash: &BlockHash,
    rights: &EndorsingRights,
    log: &Logger,
) -> Result<(), EndorsementValidationError> {
    use tezos_messages::protocol::proto_011::operation::*;

    let start = Instant::now();

    let contents = if operation.contents.len() == 1 {
        &operation.contents[0]
    } else {
        return Err(EndorsementValidationError::InvalidContents);
    };

    match contents {
        Contents::Endorsement(EndorsementOperation { level: _ }) => {
            Err(EndorsementValidationError::UnwrappedEndorsement)
        }
        Contents::EndorsementWithSlot(EndorsementWithSlotOperation { endorsement, slot }) => {
            if operation.signature.as_ref().iter().any(|b| *b != 0)
                || operation.branch != endorsement.branch
            {
                return Err(EndorsementValidationError::InvalidEndorsementWrapper);
            }

            validate_inlined_endorsement(
                endorsement,
                block_hash,
                *slot,
                rights,
                chain_id,
                start,
                log,
            )
        }
        _ => Err(EndorsementValidationError::InvalidContents),
    }
}

fn validate_inlined_endorsement(
    endorsement: &tezos_messages::protocol::proto_005_2::operation::InlinedEndorsement,
    block_hash: &BlockHash,
    slot: u16,
    rights: &EndorsingRights,
    chain_id: &ChainId,
    start: Instant,
    log: &Logger,
) -> Result<(), EndorsementValidationError> {
    if &endorsement.branch != block_hash {
        return Err(EndorsementValidationError::WrongEndorsementPredecessor);
    }
    let slot = slot as usize;
    let delegate = if slot < rights.slot_to_delegate.len() {
        &rights.slot_to_delegate[slot]
    } else {
        return Err(EndorsementValidationError::InvalidSlot);
    };
    let signature = &endorsement.signature;
    let mut encoded = endorsement.branch.as_ref().to_vec();
    let binary_endorsement = match endorsement.operations.as_bytes() {
        Ok(bytes) => bytes,
        Err(_) => return Err(EndorsementValidationError::EncodingError),
    };
    encoded.extend(binary_endorsement);
    debug!(log, "Validating endorsement";
           "binary" => FnValue(|_| hex::encode(&encoded)),
           "slot" => slot,
           "delegate" => FnValue(|_| delegate.to_string_representation()),
           "branch" => FnValue(|_| endorsement.branch.to_base58_check()),
           "contents" => FnValue(|_| format!("{:?}", endorsement.operations))
    );
    let verifying = Instant::now();
    match delegate.verify_signature(
        signature,
        &SignatureWatermark::Endorsement(chain_id.clone()),
        encoded,
    ) {
        Ok(true) => (),
        Ok(_) => return Err(EndorsementValidationError::InlinedSignatureMismatch),
        Err(CryptoError::Unsupported(_)) => {
            return Err(EndorsementValidationError::UnsupportedPublicKey)
        }
        Err(_) => return Err(EndorsementValidationError::SignatureError),
    }
    let done = Instant::now();
    debug!(log, "Endorsement signature verified";
           "total" => FnValue(|_| format!("{:?}", done - start)),
           "crypto" => FnValue(|_| format!("{:?}", done - verifying)));
    Ok(())
}
