// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Instant;

use crypto::hash::ChainId;
use slog::{debug, trace, FnValue, Logger};
use tezos_messages::{
    base::signature_public_key::SignatureWatermark, p2p::binary_message::BinaryWrite,
};

use crate::rights::{Delegate, EndorsingRights};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum EndorsementValidationError {
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
    pub protocol_data: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Refused {
    pub protocol_data: String,
    pub error: EndorsementValidationError,
}

impl Refused {
    fn new<E>(protocol_data: &str, error: E) -> Self
    where
        E: Into<EndorsementValidationError>,
    {
        Self {
            protocol_data: protocol_data.to_string(),
            error: error.into(),
        }
    }
}

pub(super) trait EndorsementValidator {
    fn validate_endorsement(
        &self,
        binary_signed_operation: &[u8],
        chain_id: &ChainId,
        rights: &EndorsingRights,
        log: &Logger,
    ) -> Result<Applied, Refused>;
}

impl EndorsementValidator for tezos_messages::protocol::proto_010::operation::Operation {
    fn validate_endorsement(
        &self,
        _binary_signed_operation: &[u8],
        chain_id: &ChainId,
        rights: &EndorsingRights,
        log: &Logger,
    ) -> Result<Applied, Refused> {
        use tezos_messages::protocol::proto_010::operation::*;

        let start = Instant::now();

        let protocol_data = match serde_json::to_string(self) {
            Ok(v) => v,
            Err(err) => return Err(Refused::new("{}", err)),
        };
        let refused = |error| Err(Refused::new(&protocol_data, error));

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
                let slot = *slot as usize;
                let delegate = if slot < rights.slot_to_delegate.len() {
                    &rights.slot_to_delegate[slot]
                } else {
                    return refused(EndorsementValidationError::InvalidSlot);
                };

                let signature = &endorsement.signature;
                let mut encoded = endorsement.branch.as_ref().to_vec();
                let binary_endorsement = match endorsement.operations.as_bytes() {
                    Ok(bytes) => bytes,
                    Err(_) => return refused(EndorsementValidationError::EncodingError),
                };
                encoded.extend(binary_endorsement);

                trace!(log, "=== Validating endorsement";
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
                    Err(_) => return refused(EndorsementValidationError::SignatureError),
                }

                let done = Instant::now();

                debug!(log, "Signature verified"; "total" => format!("{:?}", done - start), "crypto" => format!("{:?}", done - verifying));

                Ok(Applied { protocol_data })
            }
            _ => refused(EndorsementValidationError::InvalidContents),
        }
    }
}
