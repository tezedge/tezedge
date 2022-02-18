// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Operation contents. This is the contents of the opaque field [super::operation::Operation::data].
//!
//! Changes comparing to the [`proto_011::operation::Operation`](crate::proto_011::operation::Operation):
//!
//!
//! - Removed `Operation::Endorsement`
//! - Removed `Operation::EndorsementWithSlot`
//! - Added `Operation::Preendorsement`
//! - Added `operation::Operation::Endorsement`
//! - Added `Operation::DoublePreendorsementEvidence`
//! - Modified `Operation::DoubleBakingEvidence`
//! - Modified `Operation::DoubleEndorsementEvidence`
//! - Added `Operation::SetDepositsLimit`

pub use super::super::proto_011::operation::{
    ActivateAccountOperation, BallotOperation, ContractId, DelegationOperation,
    FailingNoopOperation, OriginationOperation, ProposalsOperation, RevealOperation,
    SeedNonceRevelationOperation, TransactionOperation,
};

use std::convert::TryFrom;

use crypto::hash::{
    BlockHash, BlockPayloadHash, ContextHash, HashTrait, NonceHash, OperationListListHash,
    Signature,
};
use tezos_encoding::{
    binary_reader::BinaryReaderError,
    encoding::HasEncoding,
    nom::NomReader,
    types::{Mutez, SizedBytes},
};

#[cfg(feature = "fuzzing")]
use tezos_encoding::fuzzing::sizedbytes::SizedBytesMutator;

use tezos_encoding_derive::BinWriter;

use crate::{
    base::signature_public_key::SignaturePublicKeyHash,
    p2p::encoding::{block_header::Level, fitness::Fitness, operation::Operation as P2POperation},
    protocol::proto_011::operation::RegisterGlobalConstantOperation,
    Timestamp,
};

/**
 * Operation contents.
+-----------+----------+----------------------------------------------------+
| Name      | Size     | Contents                                           |
+===========+==========+====================================================+
| branch    | 32 bytes | bytes                                              |
+-----------+----------+----------------------------------------------------+
| contents  | Variable | sequence of $012-Psithaca.operation.alpha.contents |
+-----------+----------+----------------------------------------------------+
| signature | 64 bytes | bytes                                              |
+-----------+----------+----------------------------------------------------+
 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct Operation {
    pub branch: BlockHash,
    #[encoding(reserve = "Signature::hash_size()")]
    pub contents: Vec<Contents>,
    pub signature: Option<Signature>,
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
            signature: Some(signature),
        })
    }
}

impl Operation {
    pub fn is_endorsement(&self) -> bool {
        self.as_endorsement().is_some()
    }

    pub fn as_endorsement(&self) -> Option<&EndorsementOperation> {
        if let Some((Contents::Endorsement(endorsement), [])) = self.contents.split_first() {
            Some(endorsement)
        } else {
            None
        }
    }

    pub fn is_preendorsement(&self) -> bool {
        self.as_preendorsement().is_some()
    }

    pub fn as_preendorsement(&self) -> Option<&PreendorsementOperation> {
        if let Some((Contents::Preendorsement(preendorsement), [])) = self.contents.split_first() {
            Some(preendorsement)
        } else {
            None
        }
    }

    pub fn payload(&self) -> Option<&BlockPayloadHash> {
        if let Some((contents, [])) = self.contents.split_first() {
            match contents {
                Contents::Endorsement(EndorsementOperation {
                    block_payload_hash, ..
                })
                | Contents::Preendorsement(PreendorsementOperation {
                    block_payload_hash, ..
                }) => Some(block_payload_hash),
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn level_round(&self) -> Option<(i32, i32)> {
        if let Some((contents, [])) = self.contents.split_first() {
            match contents {
                Contents::Endorsement(EndorsementOperation { level, round, .. })
                | Contents::Preendorsement(PreendorsementOperation { level, round, .. }) => {
                    Some((*level, *round))
                }
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn slot(&self) -> Option<u16> {
        if let Some((contents, [])) = self.contents.split_first() {
            match contents {
                Contents::Endorsement(EndorsementOperation { slot, .. })
                | Contents::Preendorsement(PreendorsementOperation { slot, .. }) => Some(*slot),
                _ => None,
            }
        } else {
            None
        }
    }
}

/// Operation contents.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct OperationContents {
    #[encoding(reserve = "Signature::hash_size()")]
    pub contents: Vec<Contents>,
    pub signature: Signature,
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

/**
012-Psithaca.operation.alpha.contents (Determined from data, 8-bit tag)
***********************************************************************

Seed_nonce_revelation (tag 1)
Double_endorsement_evidence (tag 2)
Double_baking_evidence (tag 3)
Activate_account (tag 4)
Proposals (tag 5)
Ballot (tag 6)
Double_preendorsement_evidence (tag 7)
Failing_noop (tag 17)
Preendorsement (tag 20)
Endorsement (tag 21)
Reveal (tag 107)
Transaction (tag 108)
Origination (tag 109)
Delegation (tag 110)
Register_global_constant (tag 111)
Set_deposits_limit (tag 112)

Changes comparing to [super::super::proto_011::operation::Contents]:
- legacy [Endorsement](super::super::proto_011::operation::Contents::Endorsement) operation is removed
- legacy [EndorsementWithSlot](super::super::proto_011::operation::Contents::EndorsementWithSlot) operation is removed
- [Contents::DoublePreendorsementEvidence] added
- [Contents::Preendorsement] added
- [Contents::Endorsement] added
- [Contents::SetDepositsLimit] added

 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
#[encoding(tags = "u8")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Contents {
    /// Seed_nonce_revelation (tag 1).
    #[encoding(tag = 1)]
    SeedNonceRevelation(SeedNonceRevelationOperation),

    /// Double_endorsement_evidence (tag 2).
    DoubleEndorsementEvidence(DoubleEndorsementEvidenceOperation),

    /// Double_baking_evidence (tag 3).
    DoubleBakingEvidence(DoubleBakingEvidenceOperation),

    /// Activate_account (tag 4).
    ActivateAccount(ActivateAccountOperation),

    /// Proposals (tag 5).
    Proposals(ProposalsOperation),

    /// Ballot (tag 6).
    Ballot(BallotOperation),

    /// Double_preendorsement_evidence (tag 7)
    DoublePreendorsementEvidence(DoublePreendorsementEvidenceOperation),

    /// Failing_noop (tag 17).
    #[encoding(tag = 17)]
    FailingNoop(FailingNoopOperation),

    /// Preendorsement (tag 20)
    #[encoding(tag = 20)]
    Preendorsement(PreendorsementOperation),

    /// Endorsement (tag 21)
    #[encoding(tag = 21)]
    Endorsement(EndorsementOperation),

    /// Reveal (tag 107).
    #[encoding(tag = 107)]
    Reveal(RevealOperation),

    /// Transaction (tag 108).
    #[encoding(tag = 108)]
    Transaction(TransactionOperation),

    /// Origination (tag 109).
    #[encoding(tag = 109)]
    Origination(OriginationOperation),

    /// Delegation (tag 110).
    #[encoding(tag = 110)]
    Delegation(DelegationOperation),

    /// Register_global_constant (tag 111).
    #[encoding(tag = 111)]
    RegisterGlobalConstant(RegisterGlobalConstantOperation),

    /// Set_deposits_limit (tag 112).
    #[encoding(tag = 112)]
    SetDepositsLimit(SetDepositsLimitOperation),
}

/**
Double_endorsement_evidence (tag 2)
===================================

+-----------------------+----------+-----------------------------------+
| Name                  | Size     | Contents                          |
+=======================+==========+===================================+
| Tag                   | 1 byte   | unsigned 8-bit integer            |
+-----------------------+----------+-----------------------------------+
| # bytes in next field | 4 bytes  | unsigned 30-bit integer           |
+-----------------------+----------+-----------------------------------+
| op1                   | Variable | $012-Psithaca.inlined.endorsement |
+-----------------------+----------+-----------------------------------+
| # bytes in next field | 4 bytes  | unsigned 30-bit integer           |
+-----------------------+----------+-----------------------------------+
| op2                   | Variable | $012-Psithaca.inlined.endorsement |
+-----------------------+----------+-----------------------------------+
 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct DoubleEndorsementEvidenceOperation {
    op1: InlinedEndorsement,
    op2: InlinedEndorsement,
}

/**
012-Psithaca.inlined.endorsement
********************************

+------------+----------+----------------------------------------------------+
| Name       | Size     | Contents                                           |
+============+==========+====================================================+
| branch     | 32 bytes | bytes                                              |
+------------+----------+----------------------------------------------------+
| operations | 43 bytes | $012-Psithaca.inlined.endorsement_mempool.contents |
+------------+----------+----------------------------------------------------+
| signature  | Variable | bytes                                              |
+------------+----------+----------------------------------------------------+
 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct InlinedEndorsement {
    branch: BlockHash,
    operations: InlinedEndorsementMempoolContents,
    signature: Signature,
}

/**
012-Psithaca.inlined.endorsement_mempool.contents (43 bytes, 8-bit tag)
***********************************************************************

Endorsement (tag 21)
 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlinedEndorsementMempoolContents {
    #[encoding(tag = 21)]
    Endorsement(InlinedEndorsementMempoolContentsEndorsementVariant),
}

/**
Endorsement (tag 21)

+--------------------+----------+-------------------------+
| Name               | Size     | Contents                |
+====================+==========+=========================+
| Tag                | 1 byte   | unsigned 8-bit integer  |
+--------------------+----------+-------------------------+
| slot               | 2 bytes  | unsigned 16-bit integer |
+--------------------+----------+-------------------------+
| level              | 4 bytes  | signed 32-bit integer   |
+--------------------+----------+-------------------------+
| round              | 4 bytes  | signed 32-bit integer   |
+--------------------+----------+-------------------------+
| block_payload_hash | 32 bytes | bytes                   |
+--------------------+----------+-------------------------+
 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct InlinedEndorsementMempoolContentsEndorsementVariant {
    pub slot: u16,
    pub level: i32,
    pub round: i32,
    pub block_payload_hash: BlockPayloadHash,
}

/// Double_baking_evidence (tag 3).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#double-baking-evidence-tag-3].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct DoubleBakingEvidenceOperation {
    #[encoding(dynamic)]
    pub bh1: FullHeader,
    #[encoding(dynamic)]
    pub bh2: FullHeader,
}

/// Full Header.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#endorsement-tag-0].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct FullHeader {
    #[encoding(builtin = "Int32")]
    pub level: Level,
    pub proto: u8,
    pub predecessor: BlockHash,
    pub timestamp: Timestamp,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    pub fitness: Fitness,
    pub context: ContextHash,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    #[cfg_attr(feature = "fuzzing", field_mutator(SizedBytesMutator<8>))]
    pub proof_of_work_nonce: SizedBytes<8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<NonceHash>,
    pub liquidity_baking_escape_vote: bool,
    pub signature: Signature,
}

/**
Double_preendorsement_evidence (tag 7)
======================================

+-----------------------+----------+--------------------------------------+
| Name                  | Size     | Contents                             |
+=======================+==========+======================================+
| Tag                   | 1 byte   | unsigned 8-bit integer               |
+-----------------------+----------+--------------------------------------+
| # bytes in next field | 4 bytes  | unsigned 30-bit integer              |
+-----------------------+----------+--------------------------------------+
| op1                   | Variable | $012-Psithaca.inlined.preendorsement |
+-----------------------+----------+--------------------------------------+
| # bytes in next field | 4 bytes  | unsigned 30-bit integer              |
+-----------------------+----------+--------------------------------------+
| op2                   | Variable | $012-Psithaca.inlined.preendorsement |
+-----------------------+----------+--------------------------------------+
*/
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct DoublePreendorsementEvidenceOperation {
    #[encoding(dynamic)]
    op1: InlinedPreendorsement,
    #[encoding(dynamic)]
    op2: InlinedPreendorsement,
}

/**
012-Psithaca.inlined.preendorsement
***********************************

+------------+----------+-----------------------------------------------+
| Name       | Size     | Contents                                      |
+============+==========+===============================================+
| branch     | 32 bytes | bytes                                         |
+------------+----------+-----------------------------------------------+
| operations | 43 bytes | $012-Psithaca.inlined.preendorsement.contents |
+------------+----------+-----------------------------------------------+
| signature  | Variable | bytes                                         |
+------------+----------+-----------------------------------------------+
 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct InlinedPreendorsement {
    branch: BlockHash,
    operations: InlinedPreendorsementContents,
    signature: Signature,
}

/**
012-Psithaca.inlined.preendorsement.contents (43 bytes, 8-bit tag)
******************************************************************

Preendorsement (tag 20)
=======================

+--------------------+----------+-------------------------+
| Name               | Size     | Contents                |
+====================+==========+=========================+
| Tag                | 1 byte   | unsigned 8-bit integer  |
+--------------------+----------+-------------------------+
| slot               | 2 bytes  | unsigned 16-bit integer |
+--------------------+----------+-------------------------+
| level              | 4 bytes  | signed 32-bit integer   |
+--------------------+----------+-------------------------+
| round              | 4 bytes  | signed 32-bit integer   |
+--------------------+----------+-------------------------+
| block_payload_hash | 32 bytes | bytes                   |
+--------------------+----------+-------------------------+
 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlinedPreendorsementContents {
    #[encoding(tag = 20)]
    Preendorsement(InlinedPreendorsementVariant),
}

/**
012-Psithaca.inlined.preendorsement.contents (43 bytes, 8-bit tag)
******************************************************************

Preendorsement (tag 20)
=======================

+--------------------+----------+-------------------------+
| Name               | Size     | Contents                |
+====================+==========+=========================+
| Tag                | 1 byte   | unsigned 8-bit integer  |
+--------------------+----------+-------------------------+
| slot               | 2 bytes  | unsigned 16-bit integer |
+--------------------+----------+-------------------------+
| level              | 4 bytes  | signed 32-bit integer   |
+--------------------+----------+-------------------------+
| round              | 4 bytes  | signed 32-bit integer   |
+--------------------+----------+-------------------------+
| block_payload_hash | 32 bytes | bytes                   |
+--------------------+----------+-------------------------+
 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct InlinedPreendorsementVariant {
    pub slot: u16,
    pub level: i32,
    pub round: i32,
    pub block_payload_hash: BlockPayloadHash,
}

/**
Preendorsement (tag 20)
=======================

+--------------------+----------+-------------------------+
| Name               | Size     | Contents                |
+====================+==========+=========================+
| Tag                | 1 byte   | unsigned 8-bit integer  |
+--------------------+----------+-------------------------+
| slot               | 2 bytes  | unsigned 16-bit integer |
+--------------------+----------+-------------------------+
| level              | 4 bytes  | signed 32-bit integer   |
+--------------------+----------+-------------------------+
| round              | 4 bytes  | signed 32-bit integer   |
+--------------------+----------+-------------------------+
| block_payload_hash | 32 bytes | bytes                   |
+--------------------+----------+-------------------------+
 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct PreendorsementOperation {
    pub slot: u16,
    pub level: i32,
    pub round: i32,
    pub block_payload_hash: BlockPayloadHash,
}

/**
012-Psithaca.inlined.endorsement_mempool.contents (43 bytes, 8-bit tag)
***********************************************************************

Endorsement (tag 21)
====================

+--------------------+----------+-------------------------+
| Name               | Size     | Contents                |
+====================+==========+=========================+
| Tag                | 1 byte   | unsigned 8-bit integer  |
+--------------------+----------+-------------------------+
| slot               | 2 bytes  | unsigned 16-bit integer |
+--------------------+----------+-------------------------+
| level              | 4 bytes  | signed 32-bit integer   |
+--------------------+----------+-------------------------+
| round              | 4 bytes  | signed 32-bit integer   |
+--------------------+----------+-------------------------+
| block_payload_hash | 32 bytes | bytes                   |
+--------------------+----------+-------------------------+
 */
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct EndorsementOperation {
    pub slot: u16,
    pub level: i32,
    pub round: i32,
    pub block_payload_hash: BlockPayloadHash,
}

/**
Set_deposits_limit (tag 112)
============================

+-----------------------------+----------------------+-------------------------------------+
| Name                        | Size                 | Contents                            |
+=============================+======================+=====================================+
| Tag                         | 1 byte               | unsigned 8-bit integer              |
+-----------------------------+----------------------+-------------------------------------+
| source                      | 21 bytes             | $public_key_hash                    |
+-----------------------------+----------------------+-------------------------------------+
| fee                         | Determined from data | $N.t                                |
+-----------------------------+----------------------+-------------------------------------+
| counter                     | Determined from data | $N.t                                |
+-----------------------------+----------------------+-------------------------------------+
| gas_limit                   | Determined from data | $N.t                                |
+-----------------------------+----------------------+-------------------------------------+
| storage_limit               | Determined from data | $N.t                                |
+-----------------------------+----------------------+-------------------------------------+
| ? presence of field "limit" | 1 byte               | boolean (0 for false, 255 for true) |
+-----------------------------+----------------------+-------------------------------------+
| limit                       | Determined from data | $N.t                                |
+-----------------------------+----------------------+-------------------------------------+
*/
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct SetDepositsLimitOperation {
    source: SignaturePublicKeyHash,
    fee: Mutez,
    counter: Mutez,
    gas_limit: Mutez,
    storage_limit: Mutez,
    limit: Option<Mutez>,
}

#[cfg(test)]
mod tests {

    use std::{fs::File, path::PathBuf};

    use anyhow::{Context, Result};

    use crate::p2p::binary_message::{BinaryRead, BinaryWrite};

    use super::Operation;

    const DATA_DIR_NAME: &str = "012_ithaca";

    fn read_data(file: &str) -> Result<serde_json::Value> {
        let dir = std::env::var("CARGO_MANIFEST_DIR")
            .with_context(|| "`CARGO_MANIFEST_DIR` is not set".to_string())?;
        let path = PathBuf::from(dir)
            .join("resources")
            .join("operations")
            .join(DATA_DIR_NAME)
            .join(file.to_string() + ".json");
        let reader = File::open(&path).with_context(|| format!("Cannot read file {path:?}"))?;
        let json = serde_json::from_reader(reader)
            .with_context(|| format!("Cannot read message from {path:?}"))?;
        Ok(json)
    }

    fn test_operation(file: &str) {
        let json = read_data(file).unwrap();
        let operation: Operation = serde_json::from_value(json.clone()).unwrap();
        let bytes = operation.as_bytes().unwrap();
        let operation1 = Operation::from_bytes(&bytes).unwrap();
        let json1 = serde_json::to_value(&operation1).unwrap();
        // println!(
        //     "<<<\n{json}\n===\n{json1}\n>>>",
        //     json = serde_json::to_string_pretty(&json).unwrap(),
        //     json1 = serde_json::to_string_pretty(&json1).unwrap(),
        // );
        assert_eq!(
            serde_json::to_string_pretty(&json).unwrap(),
            serde_json::to_string_pretty(&json1).unwrap()
        );
    }

    macro_rules! test_operations {
        ( $( $test:ident => $file:expr ),* $(,)? )=> {
            $(
                #[test]
                fn $test() {
                    test_operation($file);
                }
            )*
        }
    }

    test_operations!(
        activate_account => "operation-activate-account",
        ballot => "operation-ballot",
        delegation => "operation-delegation",
        delegation_withdrawal => "operation-delegation-withdrawal",
        double_baking_evidence => "operation-double-baking-evidence",
        double_endorsement_evidence => "operation-double-endorsement-evidence",
        endorsement => "operation-endorsement",
        endorsement_with_slot => "operation-endorsement-with-slot",
        // Disabled until contract/storage contents is implemented
        // origination => "operation-origination",
        proposals => "operation-proposals",
        reveal => "operation-reveal",
        seed_nonce_revelation => "operation-seed-nonce-revelation",
        transaction_to_implicit => "operation-transaction-to-implicit",
        // Disabled until X0 contents is implemented
        // transaction_to_originated => "operation-transaction-to-originated",
        transaction_to_originated_no_params => "operation-transaction-to-originated-no-params",
    );
}
