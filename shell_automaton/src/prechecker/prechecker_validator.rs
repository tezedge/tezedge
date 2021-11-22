// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Instant;

use crypto::hash::ChainId;
use slog::{FnValue, Logger, debug, trace};
use tezos_messages::{
    base::signature_public_key::SignatureWatermark,
    p2p::binary_message::BinaryWrite,
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
}

pub(super) trait EndorsementValidator {
    fn validate_endorsement(
        &self,
        binary_signed_operation: &[u8],
        chain_id: &ChainId,
        rights: &EndorsingRights,
        log: &Logger,
    ) -> Result<(), EndorsementValidationError>;
}

impl EndorsementValidator for tezos_messages::protocol::proto_010::operation::Operation {
    fn validate_endorsement(
        &self,
        _binary_signed_operation: &[u8],
        chain_id: &ChainId,
        rights: &EndorsingRights,
        log: &Logger,
    ) -> Result<(), EndorsementValidationError> {
        use tezos_messages::protocol::proto_010::operation::*;

        let start = Instant::now();

        let contents = if self.contents.len() == 1 {
            &self.contents[0]
        } else {
            return Err(EndorsementValidationError::InvalidContents);
        };

        match contents {
            Contents::Endorsement(EndorsementOperation { level: _ }) => {
                Err(EndorsementValidationError::UnwrappedEndorsement)
            }
            Contents::EndorsementWithSlot(EndorsementWithSlotOperation { endorsement, slot }) => {
                let slot = *slot as usize;
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
                    Ok(_) => return Err(EndorsementValidationError::InlinedSignatureMismatch),
                    Err(_) => return Err(EndorsementValidationError::SignatureError),
                }

                let done = Instant::now();

                debug!(log, "Signature verified"; "total" => format!("{:?}", done - start), "crypto" => format!("{:?}", done - verifying));

                Ok(())
            }
            _ => return Err(EndorsementValidationError::InvalidContents),
        }
    }
}
