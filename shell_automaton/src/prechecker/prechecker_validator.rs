// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::rights::EndorsingRights;

enum EndorsementValidationError {
    #[error("Delegate {0:?} does not have endorsing rights")]
    NoEndorsingRights(Delegate),
    #[error("Failed to verify the operation's signature")]
    SignatureError,
    #[error("Failed to verify the operation's inlined signature")]
    InlinedSignatureError,
}

pub(crate) trait EndorsementValidator {
    fn validate(&self, rights: &EndorsingRights) -> Result<(), EndorsementValidationError>;
}

impl EndorsementValidator for tezos_messages::protocol::proto_010::operation::OperationContents {
    fn validate(&self, rights: &EndorsingRights) -> Result<(), EndorsementValidationError> {
        let signature = &self.signature;
        for (delegate, _) in rights.delegate_to_slots {

        }
    }
}
