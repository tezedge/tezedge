// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Operation contents. This is the contents of the opaque field [super::operation::Operation::data].
//! See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-operation-alpha-contents-determined-from-data-8-bit-tag].

pub use super::super::proto_011::operation::{
    ActivateAccountOperation, BallotOperation, ContractId, DelegationOperation,
    DoubleEndorsementEvidenceOperation, EndorsementOperation, EndorsementWithSlotOperation,
    FailingNoopOperation, InlinedEndorsement, InlinedEndorsementContents,
    InlinedEndorsementVariant, Operation, OperationContents, OriginationOperation,
    ProposalsOperation, RegisterGlobalConstantOperation, RevealOperation,
    SeedNonceRevelationOperation, TransactionOperation,
};

use crypto::hash::{BlockHash, BlockPayloadHash, ContextHash, OperationListListHash, Signature};
use tezos_encoding::{encoding::HasEncoding, nom::NomReader};

use crate::p2p::encoding::{
    block_header::{Fitness, Level},
    limits::BLOCK_HEADER_FITNESS_MAX_SIZE,
};

/// Operation contents.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-operation-alpha-contents-determined-from-data-8-bit-tag].
///
/// Comparing to [super::super::proto_010::operation::Content], nothing was added, but the header changed some fields (and is used by `DoubleBakingEvidenceOperation`).
#[derive(Debug, Clone, HasEncoding, NomReader)]
#[encoding(tags = "u8")]
pub enum Contents {
    /// Endorsmnent (tag 0).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#id5].
    Endorsement(EndorsementOperation),

    /// Seed_nonce_revelation (tag 1).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#seed-nonce-revelation-tag-1].
    SeedNonceRevelation(SeedNonceRevelationOperation),

    /// Double_endorsement_evidence (tag 2).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#double-endorsement-evidence-tag-2].
    DoubleEndorsementEvidence(DoubleEndorsementEvidenceOperation),

    /// Double_baking_evidence (tag 3).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#double-baking-evidence-tag-3].
    DoubleBakingEvidence(DoubleBakingEvidenceOperation),

    /// Activate_account (tag 4).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#activate-account-tag-4].
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
    EndorsementWithSlot(EndorsementWithSlotOperation),

    /// Failing_noop (tag 17).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#failing-noop-tag-17].
    #[encoding(tag = 17)]
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
#[derive(Debug, Clone, HasEncoding, NomReader)]
pub struct DoubleBakingEvidenceOperation {
    #[encoding(dynamic)]
    pub bh1: FullHeader,
    #[encoding(dynamic)]
    pub bh2: FullHeader,
}

/// Full Header.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#endorsement-tag-0].
/// Compared to the earlier version, `priority` got removed and `payload_hash` and `payload_round` got added.
#[derive(Debug, Clone, HasEncoding, NomReader)]
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
