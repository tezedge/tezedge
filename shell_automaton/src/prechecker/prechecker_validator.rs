// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Instant;

use crypto::{
    hash::{BlockHash, ChainId},
    CryptoError,
};
use slog::{trace, FnValue, Logger};
use tezos_messages::{
    base::signature_public_key::SignatureWatermark,
    p2p::{binary_message::BinaryWrite, encoding::block_header::Level},
};

use crate::rights::{Delegate, EndorsingRights};

use super::OperationDecodedContents;

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
    pub protocol_data: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Refused {
    pub protocol_data: serde_json::Value,
    pub error: EndorsementValidationError,
}

impl Refused {
    fn new<E>(protocol_data: serde_json::Value, error: E) -> Self
    where
        E: Into<EndorsementValidationError>,
    {
        Self {
            protocol_data,
            error: error.into(),
        }
    }
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
        serde_json::to_value(self).unwrap_or(serde_json::json!("cannot convert to json"))
    }
}

pub(super) trait EndorsementValidator {
    fn validate_endorsement(
        &self,
        binary_signed_operation: &[u8],
        chain_id: &ChainId,
        block_hash: &BlockHash,
        rights: &EndorsingRights,
        log: &Logger,
    ) -> Result<Applied, Refused>;
}

impl EndorsementValidator for OperationDecodedContents {
    fn validate_endorsement(
        &self,
        binary_signed_operation: &[u8],
        chain_id: &ChainId,
        block_hash: &BlockHash,
        rights: &EndorsingRights,
        log: &Logger,
    ) -> Result<Applied, Refused> {
        match self {
            OperationDecodedContents::Proto010(operation) => operation.validate_endorsement(
                binary_signed_operation,
                chain_id,
                block_hash,
                rights,
                log,
            ),
            OperationDecodedContents::Proto011(operation) => operation.validate_endorsement(
                binary_signed_operation,
                chain_id,
                block_hash,
                rights,
                log,
            ),
        }
    }
}

impl EndorsementValidator for tezos_messages::protocol::proto_010::operation::Operation {
    fn validate_endorsement(
        &self,
        _binary_signed_operation: &[u8],
        chain_id: &ChainId,
        block_hash: &BlockHash,
        rights: &EndorsingRights,
        log: &Logger,
    ) -> Result<Applied, Refused> {
        use tezos_messages::protocol::proto_010::operation::*;

        let start = Instant::now();

        let refused = |error| Err(Refused::new(self.as_json(), error));

        let contents = if self.contents.len() == 1 {
            &self.contents[0]
        } else {
            return refused(EndorsementValidationError::InvalidContents);
        };

        match contents {
            Contents::Endorsement(EndorsementOperation { level: _ }) => {
                refused(EndorsementValidationError::UnwrappedEndorsement)
            }
            Contents::EndorsementWithSlot(EndorsementWithSlotOperation { endorsement, slot }) => {
                if self.signature.as_ref().iter().any(|b| *b != 0)
                    || self.branch != endorsement.branch
                {
                    return refused(EndorsementValidationError::InvalidEndorsementWrapper);
                }

                if &endorsement.branch.0 != block_hash {
                    return refused(EndorsementValidationError::WrongEndorsementPredecessor);
                }

                let slot = *slot as usize;
                let delegate = if slot < rights.slot_to_delegate.len() {
                    &rights.slot_to_delegate[slot]
                } else {
                    return refused(EndorsementValidationError::InvalidSlot);
                };

                let signature = &endorsement.signature.0;
                let mut encoded = endorsement.branch.as_ref().to_vec();
                let binary_endorsement = match endorsement.operations.as_bytes() {
                    Ok(bytes) => bytes,
                    Err(_) => return refused(EndorsementValidationError::EncodingError),
                };
                encoded.extend(binary_endorsement);

                trace!(log, "Validating endorsement";
                       "binary" => FnValue(|_| hex::encode(&encoded)),
                       "slot" => slot,
                       "delegate" => FnValue(|_| delegate.to_string_representation()),
                       "branch" => FnValue(|_| endorsement.branch.to_base58_check()),
                       "contents" => FnValue(|_| format!("{:?}", endorsement.operations))
                );

                let verifying = Instant::now();

                match delegate.verify_signature(
                    signature,
                    SignatureWatermark::Endorsement(chain_id.clone()),
                    encoded,
                ) {
                    Ok(true) => (),
                    Ok(_) => return refused(EndorsementValidationError::InlinedSignatureMismatch),
                    Err(CryptoError::Unsupported(_)) => {
                        return refused(EndorsementValidationError::UnsupportedPublicKey)
                    }
                    Err(_) => return refused(EndorsementValidationError::SignatureError),
                }

                let done = Instant::now();

                trace!(log, "Endorsement signature verified";
                       "total" => FnValue(|_| format!("{:?}", done - start)),
                       "crypto" => FnValue(|_| format!("{:?}", done - verifying)),
                       "json" => FnValue(|_| self.as_json().to_string()));

                Ok(Applied {
                    protocol_data: self.as_json(),
                })
            }
            _ => refused(EndorsementValidationError::InvalidContents),
        }
    }
}
