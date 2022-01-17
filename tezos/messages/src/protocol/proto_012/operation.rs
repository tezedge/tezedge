// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Operation contents. This is the contents of the opaque field [super::operation::Operation::data].
//!
//! See https://tezos.gitlab.io/protocols/012_ithaca.html

pub use super::super::proto_011::operation::{
    ActivateAccountOperation, BallotOperation, ContractId, DelegationOperation,
    DoubleEndorsementEvidenceOperation, EndorsementOperation, EndorsementWithSlotOperation,
    FailingNoopOperation, InlinedEndorsement, InlinedEndorsementContents,
    InlinedEndorsementVariant, OriginationOperation, ProposalsOperation,
    RegisterGlobalConstantOperation, RevealOperation, SeedNonceRevelationOperation,
    TransactionOperation,
};

use crypto::hash::{BlockHash, ContextHash, HashTrait, OperationListListHash, Signature};
use std::convert::TryFrom;

use crypto::hash::BlockPayloadHash;
use tezos_encoding::{binary_reader::BinaryReaderError, encoding::HasEncoding, nom::NomReader};

use crate::p2p::encoding::{
    block_header::{Fitness, Level},
    limits::BLOCK_HEADER_FITNESS_MAX_SIZE,
    operation::Operation as P2POperation,
};

/// Operation contents.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#operation-alpha-specific].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding)]
pub struct Operation {
    pub branch: BlockHash,
    pub contents: Vec<Contents>,
    pub signature: Signature,
}

impl tezos_encoding::nom::NomReader for Operation {
    fn nom_read(bytes: &[u8]) -> tezos_encoding::nom::NomResult<Self> {
        nom::combinator::map(
            nom::sequence::tuple((
                tezos_encoding::nom::field(
                    "Operation::branch",
                    <BlockHash as tezos_encoding::nom::NomReader>::nom_read,
                ),
                tezos_encoding::nom::field(
                    "Operation::contents",
                    tezos_encoding::nom::reserve(
                        Signature::hash_size(),
                        tezos_encoding::nom::list(
                            <Contents as tezos_encoding::nom::NomReader>::nom_read,
                        ),
                    ),
                ),
                tezos_encoding::nom::field(
                    "Operation::signature",
                    <Signature as tezos_encoding::nom::NomReader>::nom_read,
                ),
            )),
            |(branch, contents, signature)| Operation {
                branch,
                contents,
                signature,
            },
        )(bytes)
    }
}

impl TryFrom<P2POperation> for Operation {
    type Error = BinaryReaderError;

    fn try_from(operation: P2POperation) -> Result<Self, Self::Error> {
        use crate::p2p::binary_message::BinaryRead;
        let branch = operation.branch().clone();
        let OperationContents {
            contents,
            signature,
        } = OperationContents::from_bytes(operation.data())?;
        Ok(Operation {
            branch,
            contents,
            signature,
        })
    }
}

/// Operation contents.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#operation-alpha-specific].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding)]
pub struct OperationContents {
    pub contents: Vec<Contents>,
    pub signature: Signature,
}

impl tezos_encoding::nom::NomReader for OperationContents {
    fn nom_read(bytes: &[u8]) -> tezos_encoding::nom::NomResult<Self> {
        nom::combinator::map(
            nom::sequence::tuple((
                tezos_encoding::nom::field(
                    "OperationContents::contents",
                    tezos_encoding::nom::reserve(
                        Signature::hash_size(),
                        tezos_encoding::nom::list(
                            <Contents as tezos_encoding::nom::NomReader>::nom_read,
                        ),
                    ),
                ),
                tezos_encoding::nom::field(
                    "OperationContents::signature",
                    <Signature as tezos_encoding::nom::NomReader>::nom_read,
                ),
            )),
            |(contents, signature)| OperationContents {
                contents,
                signature,
            },
        )(bytes)
    }
}

impl TryFrom<P2POperation> for OperationContents {
    type Error = BinaryReaderError;

    fn try_from(operation: P2POperation) -> Result<Self, Self::Error> {
        use crate::p2p::binary_message::BinaryRead;
        let OperationContents {
            contents,
            signature,
        } = OperationContents::from_bytes(operation.data())?;
        Ok(OperationContents {
            contents,
            signature,
        })
    }
}

/// Operation contents.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-operation-alpha-contents-determined-from-data-8-bit-tag].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader)]
#[encoding(tags = "u8")]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Contents {
    /// Endorsmnent (tag 0).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#id5].
    Endorsement(EndorsementOperation),

    /// Seed_nonce_revelation (tag 1).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#seed-nonce-revelation-tag-1].
    #[serde(rename = "seed_nonce_revelation")]
    SeedNonceRevelation(SeedNonceRevelationOperation),

    /// Double_endorsement_evidence (tag 2).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#double-endorsement-evidence-tag-2].
    #[serde(rename = "double_endorsement_evidence")]
    DoubleEndorsementEvidence(DoubleEndorsementEvidenceOperation),

    /// Double_baking_evidence (tag 3).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#double-baking-evidence-tag-3].
    #[serde(rename = "double_baking_evidence")]
    DoubleBakingEvidence(DoubleBakingEvidenceOperation),

    /// Activate_account (tag 4).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#activate-account-tag-4].
    #[serde(rename = "activate_account")]
    ActivateAccount(ActivateAccountOperation),

    /// Proposals (tag 5).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#proposals-tag-5].
    Proposals(ProposalsOperation),

    /// Ballot (tag 6).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#ballot-tag-6].
    Ballot(BallotOperation),

    /// Endorsement_with_slot (tag 10).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#endorsement-with-slot-tag-10].
    #[encoding(tag = 10)]
    #[serde(rename = "endorsement_with_slot")]
    EndorsementWithSlot(EndorsementWithSlotOperation),

    /// Failing_noop (tag 17).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#failing-noop-tag-17].
    #[encoding(tag = 17)]
    #[serde(rename = "failing_noop")]
    FailingNoop(FailingNoopOperation),

    /// Reveal (tag 107).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#reveal-tag-107].
    #[encoding(tag = 107)]
    Reveal(RevealOperation),
    /// Transaction (tag 108).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#transaction-tag-108].
    #[encoding(tag = 108)]
    Transaction(TransactionOperation),
    /// Origination (tag 109).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#origination-tag-109].
    #[encoding(tag = 109)]
    Origination(OriginationOperation),
    /// Delegation (tag 110).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#delegation-tag-110].
    #[encoding(tag = 110)]
    Delegation(DelegationOperation),

    /// Register_global_constant (tag 111).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#register-global-constant-tag-111].
    #[encoding(tag = 111)]
    RegisterGlobalConstant(RegisterGlobalConstantOperation),
}

/// Double_baking_evidence (tag 3).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#double-baking-evidence-tag-3].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader)]
pub struct DoubleBakingEvidenceOperation {
    #[encoding(dynamic)]
    pub bh1: FullHeader,
    #[encoding(dynamic)]
    pub bh2: FullHeader,
}

/// Full Header.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#endorsement-tag-0].
/// Compared to the earlier version, `priority` got removed and `payload_hash` and `payload_round` got added.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader)]
pub struct FullHeader {
    #[encoding(builtin = "Int32")]
    pub level: Level,
    pub proto: u8,
    pub predecessor: BlockHash,
    #[encoding(timestamp)]
    pub timestamp: i64,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    #[encoding(composite(
        dynamic = "BLOCK_HEADER_FITNESS_MAX_SIZE",
        list,
        dynamic,
        list,
        builtin = "Uint8"
    ))]
    pub fitness: Fitness,
    pub context: ContextHash,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    #[encoding(sized = "8", bytes)]
    pub proof_of_work_nonce: Vec<u8>,
    #[encoding(option, sized = "32", bytes)]
    pub seed_nonce_hash: Option<Vec<u8>>,
    pub liquidity_baking_escape_vote: bool,
    pub signature: Signature,
}
