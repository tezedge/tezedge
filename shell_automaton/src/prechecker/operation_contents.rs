// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, BlockPayloadHash, ChainId};
use tezos_messages::{
    base::signature_public_key::SignaturePublicKey,
    p2p::encoding::operation::Operation,
    protocol::{
        proto_012::operation::OperationVerifyError, SupportedProtocol, UnsupportedProtocolError,
    },
};

use crate::rights::Slot;

use super::{OperationProtocolData, PrecheckerError, TenderbakeConsensusContents};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum OperationDecodedContents {
    Proto010(tezos_messages::protocol::proto_010::operation::Operation),
    Proto011(tezos_messages::protocol::proto_011::operation::Operation),
    Proto012(tezos_messages::protocol::proto_012::operation::Operation),
}

impl OperationDecodedContents {
    pub(super) fn verify_signature(
        &self,
        pk: &SignaturePublicKey,
        chain_id: &ChainId,
    ) -> Result<bool, OperationVerifyError> {
        match self {
            OperationDecodedContents::Proto012(operation) => {
                operation.verify_signature(pk, chain_id)
            }
            _ => Err(OperationVerifyError::InvalidContents),
        }
    }

    pub(super) fn parse(
        shell_operation: &Operation,
        proto: &SupportedProtocol,
    ) -> Result<Self, PrecheckerError> {
        use tezos_messages::protocol::FromShell;
        Ok(match proto {
            SupportedProtocol::Proto010 => Self::Proto010(
                tezos_messages::protocol::proto_010::operation::Operation::convert_from(
                    shell_operation,
                )?,
            ),
            SupportedProtocol::Proto011 => Self::Proto011(
                tezos_messages::protocol::proto_011::operation::Operation::convert_from(
                    shell_operation,
                )?,
            ),
            SupportedProtocol::Proto012 => Self::Proto012(
                tezos_messages::protocol::proto_012::operation::Operation::convert_from(
                    shell_operation,
                )?,
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

    pub(crate) fn as_tenderbake_consensus(&self) -> Option<TenderbakeConsensusContents> {
        if let OperationDecodedContents::Proto012(operation) = self {
            if let (Some((level, round)), Some(payload_hash), Some(slot)) = (
                operation.level_round(),
                operation.payload().cloned(),
                operation.slot(),
            ) {
                Some(TenderbakeConsensusContents {
                    level,
                    round,
                    payload_hash,
                    slot,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}
