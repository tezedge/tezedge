// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Operation contents. This is the contents of the opaque field [super::operation::Operation::data].
//! See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-operation-alpha-contents-determined-from-data-8-bit-tag].

use std::convert::TryFrom;

use serde::{Deserialize, Serialize};

use crypto::hash::{
    BlockHash, ContextHash, ContractKt1Hash, ContractTz1Hash, HashTrait, OperationListListHash,
    ProtocolHash, Signature,
};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_encoding::types::{Mutez, SizedBytes};
use tezos_encoding::{enc::BinWriter, encoding::HasEncoding, nom::NomReader};

#[cfg(feature = "fuzzing")]
use tezos_encoding::fuzzing::sizedbytes::SizedBytesMutator;

#[cfg(feature = "fuzzing")]
use fuzzcheck::mutators::option::OptionMutator;

use crate::base::signature_public_key::{SignaturePublicKey, SignaturePublicKeyHash};
use crate::Timestamp;

use crate::p2p::encoding::{
    block_header::Level, fitness::Fitness, operation::Operation as P2POperation,
};

/// Operation contents.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#operation-alpha-specific].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct Operation {
    pub branch: BlockHash,
    #[encoding(reserve = "Signature::hash_size()")]
    pub contents: Vec<Contents>,
    pub signature: Signature,
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

//==============================

/// Inline endorsement content, Endorsement (tag 0).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#endorsement-tag-0].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, Serialize, Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct InlinedEndorsementVariant {
    pub level: i32,
}

/// Inlined endorsement contents.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-inlined-endorsement-contents-5-bytes-8-bit-tag].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, Serialize, Deserialize, HasEncoding, NomReader, BinWriter)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum InlinedEndorsementContents {
    /// Endorsement (tag 0).
    /// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#endorsement-tag-0].
    Endorsement(InlinedEndorsementVariant),
}

/// Inlined endorsement.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-inlined-endorsement].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct InlinedEndorsement {
    pub branch: BlockHash,
    pub operations: InlinedEndorsementContents,
    pub signature: Signature,
}

//==============================

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
    pub priority: u16,
    #[cfg_attr(feature = "fuzzing", field_mutator(SizedBytesMutator<8>))]
    pub proof_of_work_nonce: SizedBytes<8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "fuzzing", field_mutator(OptionMutator<SizedBytes<32>, SizedBytesMutator<32>>))]
    pub seed_nonce_hash: Option<SizedBytes<32>>,
    pub signature: Signature,
}

//==============================

/// Operation contents.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-operation-alpha-contents-determined-from-data-8-bit-tag].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
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
}

/// Endorsmnent (tag 0).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#id5].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct EndorsementOperation {
    #[encoding(builtin = "Int32")]
    pub level: Level,
}

/// Seed_nonce_revelation (tag 1).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#seed-nonce-revelation-tag-1].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct SeedNonceRevelationOperation {
    pub level: i32,
    #[cfg_attr(feature = "fuzzing", field_mutator(SizedBytesMutator<32>))]
    pub nonce: SizedBytes<32>,
}

/// Double_endorsement_evidence (tag 2).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#double-endorsement-evidence-tag-2].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct DoubleEndorsementEvidenceOperation {
    #[encoding(dynamic)]
    pub op1: InlinedEndorsement,
    #[encoding(dynamic)]
    pub op2: InlinedEndorsement,
}

/// Double_baking_evidence (tag 3).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#double-baking-evidence-tag-3].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct DoubleBakingEvidenceOperation {
    #[encoding(dynamic)]
    pub bh1: FullHeader,
    #[encoding(dynamic)]
    pub bh2: FullHeader,
}

/// Activate_account (tag 4).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#activate-account-tag-4].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct ActivateAccountOperation {
    pub pkh: ContractTz1Hash,
    #[cfg_attr(feature = "fuzzing", field_mutator(SizedBytesMutator<20>))]
    pub secret: SizedBytes<20>,
}

/// Proposals (tag 5).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#proposals-tag-5].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct ProposalsOperation {
    pub source: SignaturePublicKeyHash,
    pub period: i32,
    #[encoding(dynamic, list)]
    pub proposals: Vec<ProtocolHash>,
}

/// Ballot (tag 6).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#ballot-tag-6].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct BallotOperation {
    pub source: SignaturePublicKeyHash,
    pub period: i32,
    pub proposal: ProtocolHash,
    pub ballot: Ballot,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
#[serde(rename_all = "snake_case")]
pub enum Ballot {
    Nay,
    Pass,
    Yay,
}

/// Reveal (tag 107).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#reveal-tag-107].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct RevealOperation {
    pub source: SignaturePublicKeyHash,
    pub fee: Mutez,
    pub counter: Mutez,
    pub gas_limit: Mutez,
    pub storage_limit: Mutez,
    pub public_key: SignaturePublicKey,
}

/// Transaction (tag 108).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#transaction-tag-108].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct TransactionOperation {
    pub source: SignaturePublicKeyHash,
    pub fee: Mutez,
    pub counter: Mutez,
    pub gas_limit: Mutez,
    pub storage_limit: Mutez,
    pub amount: Mutez,
    pub destination: ContractId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<X0>,
}

/// Origination (tag 109).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#origination-tag-109].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct OriginationOperation {
    pub source: SignaturePublicKeyHash,
    pub fee: Mutez,
    pub counter: Mutez,
    pub gas_limit: Mutez,
    pub storage_limit: Mutez,
    pub balance: Mutez,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegate: Option<SignaturePublicKeyHash>,
    pub script: ScriptedContract,
}

/// Delegation (tag 110).
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#delegation-tag-110].
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct DelegationOperation {
    pub source: SignaturePublicKeyHash,
    pub fee: Mutez,
    pub counter: Mutez,
    pub gas_limit: Mutez,
    pub storage_limit: Mutez,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegate: Option<SignaturePublicKeyHash>,
}

// ======================================

/// X_0.
/// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#x-0.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct X0 {
    pub entrypoint: Entrypoint,
    #[encoding(dynamic, bytes)]
    pub value: Vec<u8>,
}

/// alpha.entrypoint.
/// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-entrypoint-determined-from-data-8-bit-tag.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub enum Entrypoint {
    /// default (tag 0).
    /// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#default-tag-0.
    Default,

    /// root (tag 1).
    /// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#root-tag-1.
    Root,

    /// do (tag 2).
    /// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#do-tag-2.
    Do,

    /// set_delegate (tag 3).
    /// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#set-delegate-tag-3.
    SetDelegate,

    /// remove_delegate (tag 4).
    /// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#remove-delegate-tag-4.
    RemoveDelegate,

    /// named (tag 255).
    /// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#named-tag-255.
    #[encoding(tag = 255)]
    Named(ShortDynamicData),
}

/// .
/// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#named-tag-255.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct ShortDynamicData {
    #[encoding(short_dynamic, bytes)]
    pub data: Vec<u8>,
}

/// alpha.scripted.contracts.
/// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-scripted-contracts.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct ScriptedContract {
    #[encoding(dynamic, bytes)]
    pub code: Vec<u8>,
    #[encoding(dynamic, bytes)]
    pub storage: Vec<u8>,
}

/// alpha.contract_id (22 bytes, 8-bit tag).
/// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-contract-id-22-bytes-8-bit-tag.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, HasEncoding, NomReader, BinWriter)]
pub enum ContractId {
    /// Implicit (tag 0).
    /// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#implicit-tag-0.
    Implicit(SignaturePublicKeyHash),

    /// Originated (tag 1).
    /// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#originated-tag-1.
    Originated(OriginatedContractId),
}

impl Serialize for ContractId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ContractId::Implicit(v) => v.serialize(serializer),
            ContractId::Originated(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ContractId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.starts_with("tz1") | s.starts_with("tz2") | s.starts_with("tz3") {
            SignaturePublicKeyHash::from_b58_hash(&s)
                .map_err(serde::de::Error::custom)
                .map(ContractId::Implicit)
        } else if s.starts_with("KT1") {
            Ok(ContractId::Originated(OriginatedContractId {
                contract_hash: ContractKt1Hash::from_b58check(&s)
                    .map_err(serde::de::Error::custom)?,
                padding: 0,
            }))
        } else {
            unimplemented!()
        }
    }
}

/// Originated (tag 1).
/// See https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#originated-tag-1.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
#[serde(from = "ContractKt1Hash", into = "ContractKt1Hash")]
pub struct OriginatedContractId {
    pub contract_hash: ContractKt1Hash,
    pub padding: u8,
}

impl From<OriginatedContractId> for ContractKt1Hash {
    fn from(source: OriginatedContractId) -> Self {
        source.contract_hash
    }
}

impl From<ContractKt1Hash> for OriginatedContractId {
    fn from(contract_hash: ContractKt1Hash) -> Self {
        Self {
            contract_hash,
            padding: 0,
        }
    }
}

#[cfg(test)]
mod tests {

    use std::{fs::File, io::Read, path::PathBuf};

    use anyhow::{Context, Result};
    use num_bigint::BigInt;

    use crate::p2p::binary_message::BinaryRead;
    use crate::p2p::encoding::block_header::display_fitness;
    use crate::p2p::encoding::operation::Operation as P2POperation;

    use super::*;

    const DATA_DIR_NAME: &str = "001_older";

    fn read_data(file: &str) -> Result<Vec<u8>> {
        let dir = std::env::var("CARGO_MANIFEST_DIR")
            .with_context(|| "`CARGO_MANIFEST_DIR` is not set".to_string())?;
        let path = PathBuf::from(dir)
            .join("resources")
            .join("operations")
            .join(DATA_DIR_NAME)
            .join(file.to_string() + ".bin");
        let data = File::open(&path)
            .and_then(|mut file| {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                Ok(data)
            })
            .with_context(|| format!("Cannot read message from {}", path.to_string_lossy()))?;
        Ok(data)
    }

    macro_rules! operation_contents_test {
	    ($name:ident, $branch:literal, $signature:literal, $contents:ident, $contents_assert:block) => {
            #[test]
            fn $name() -> Result<()> {
                use std::convert::TryInto;

                let bytes = read_data(stringify!($name))?;

                let Operation {
                    branch,
                    $contents,
                    signature,
                } = Operation::from_bytes(&bytes)?;
                assert_eq!(
                    branch.to_base58_check(),
                    $branch
                );
                assert_eq!(signature.to_base58_check(), $signature);
                assert_eq!($contents.len(), 1);
                $contents_assert;

                let operation = P2POperation::from_bytes(&bytes)?;
                let Operation {
                    branch,
                    $contents,
                    signature,
                } = operation.try_into()?;
                assert_eq!(
                    branch.to_base58_check(),
                    $branch
                );
                assert_eq!(signature.to_base58_check(), $signature);
                assert_eq!($contents.len(), 1);
                $contents_assert;

                let operation = P2POperation::from_bytes(&bytes)?;
                let OperationContents {
                    $contents,
                    signature,
                } = operation.clone().try_into()?;
                assert_eq!(signature.to_base58_check(), $signature);
                assert_eq!($contents.len(), 1);
                $contents_assert;

                Ok(())
            }
	    };
	    ($name:ident, $contents:ident, $contents_assert:block) => {
            operation_contents_test!($name, "BKpbfCvh777DQHnXjU2sqHvVUNZ7dBAdqEfKkdw8EGSkD9LSYXb", "sigbQ5ZNvkjvGssJgoAnUAfY4Wvvg3QZqawBYB1j1VDBNTMBAALnCzRHWzer34bnfmzgHg3EvwdzQKdxgSghB897cono6gbQ", $contents, $contents_assert);
        };
    }

    operation_contents_test!(endorsement, contents, {
        match contents[0] {
            Contents::Endorsement(EndorsementOperation { level }) => assert_eq!(level, 1331),
            _ => panic!("endorsement expected"),
        }
    });

    operation_contents_test!(seed_nonce_revelation, contents, {
        match &contents[0] {
            Contents::SeedNonceRevelation(SeedNonceRevelationOperation { level, nonce }) => {
                assert_eq!(*level, 1331);
                assert_eq!(nonce, &[0; 32].into());
            }
            _ => panic!("seed nonce revelation expected"),
        }
    });

    operation_contents_test!(double_endorsement_evidence, contents, {
        match &contents[0] {
            Contents::DoubleEndorsementEvidence(DoubleEndorsementEvidenceOperation {
                op1,
                op2,
            }) => {
                assert_eq!(
                    op1.branch.to_base58_check(),
                    "BKpbfCvh777DQHnXjU2sqHvVUNZ7dBAdqEfKkdw8EGSkD9LSYXb"
                );
                assert_eq!(
                    op2.branch.to_base58_check(),
                    "BKpbfCvh777DQHnXjU2sqHvVUNZ7dBAdqEfKkdw8EGSkD9LSYXb"
                );
                assert!(matches!(
                    op1.operations,
                    InlinedEndorsementContents::Endorsement(InlinedEndorsementVariant {
                        level: 1331
                    })
                ));
                assert!(matches!(
                    op2.operations,
                    InlinedEndorsementContents::Endorsement(InlinedEndorsementVariant {
                        level: 1331
                    })
                ));
                assert_eq!(op1.signature.to_base58_check(), "sigbQ5ZNvkjvGssJgoAnUAfY4Wvvg3QZqawBYB1j1VDBNTMBAALnCzRHWzer34bnfmzgHg3EvwdzQKdxgSghB897cono6gbQ");
                assert_eq!(op2.signature.to_base58_check(), "sigbQ5ZNvkjvGssJgoAnUAfY4Wvvg3QZqawBYB1j1VDBNTMBAALnCzRHWzer34bnfmzgHg3EvwdzQKdxgSghB897cono6gbQ");
            }
            _ => panic!("double endorsement evidence expected"),
        }
    });

    operation_contents_test!(double_baking_evidence, contents, {
        match &contents[0] {
            Contents::DoubleBakingEvidence(DoubleBakingEvidenceOperation { bh1, bh2 }) => {
                assert_eq!(bh1.level, 1331);
                assert_eq!(bh1.proto, 1);
                assert_eq!(
                    bh1.predecessor.to_base58_check(),
                    "BKpbfCvh777DQHnXjU2sqHvVUNZ7dBAdqEfKkdw8EGSkD9LSYXb"
                );
                // TODO assert_eq!(bh1.timestamp, "2020-04-20T16:20:00Z");
                assert_eq!(bh1.validation_pass, 4);
                assert_eq!(
                    bh1.operations_hash.to_base58_check(),
                    "LLoZqBDX1E2ADRXbmwYo8VtMNeHG6Ygzmm4Zqv97i91UPBQHy9Vq3"
                );
                assert_eq!(display_fitness(&bh1.fitness), "01::000000000000000a");
                assert_eq!(
                    bh1.context.to_base58_check(),
                    "CoVDyf9y9gHfAkPWofBJffo4X4bWjmehH2LeVonDcCKKzyQYwqdk"
                );
                assert_eq!(bh1.priority, 0);
                assert_eq!(hex::encode(&bh1.proof_of_work_nonce), "101895ca00000000");
                assert_eq!(bh1.signature.to_base58_check(), "sigbQ5ZNvkjvGssJgoAnUAfY4Wvvg3QZqawBYB1j1VDBNTMBAALnCzRHWzer34bnfmzgHg3EvwdzQKdxgSghB897cono6gbQ");

                assert_eq!(bh2.level, 1331);
                assert_eq!(bh2.proto, 1);
                assert_eq!(
                    bh2.predecessor.to_base58_check(),
                    "BKpbfCvh777DQHnXjU2sqHvVUNZ7dBAdqEfKkdw8EGSkD9LSYXb"
                );
                // TODO assert_eq!(bh2.timestamp, "2020-04-20T16:20:00Z");
                assert_eq!(bh2.validation_pass, 4);
                assert_eq!(
                    bh2.operations_hash.to_base58_check(),
                    "LLoZqBDX1E2ADRXbmwYo8VtMNeHG6Ygzmm4Zqv97i91UPBQHy9Vq3"
                );
                assert_eq!(display_fitness(&bh2.fitness), "01::000000000000000a");
                assert_eq!(
                    bh2.context.to_base58_check(),
                    "CoVDyf9y9gHfAkPWofBJffo4X4bWjmehH2LeVonDcCKKzyQYwqdk"
                );
                assert_eq!(bh2.priority, 0);
                assert_eq!(hex::encode(&bh2.proof_of_work_nonce), "101895ca00000000");
                assert_eq!(bh2.signature.to_base58_check(), "sigbQ5ZNvkjvGssJgoAnUAfY4Wvvg3QZqawBYB1j1VDBNTMBAALnCzRHWzer34bnfmzgHg3EvwdzQKdxgSghB897cono6gbQ");
            }
            _ => panic!("double baking evidence expected"),
        }
    });

    operation_contents_test!(activate_account, contents, {
        match &contents[0] {
            Contents::ActivateAccount(ActivateAccountOperation { pkh, secret }) => {
                assert_eq!(
                    pkh.to_base58_check(),
                    "tz1ddb9NMYHZi5UzPdzTZMYQQZoMub195zgv"
                );
                assert_eq!(
                    hex::encode(&secret),
                    "41f98b15efc63fa893d61d7d6eee4a2ce9427ac4"
                );
            }
            _ => panic!("activate account expected"),
        }
    });

    operation_contents_test!(proposals, contents, {
        match &contents[0] {
            Contents::Proposals(ProposalsOperation {
                source,
                period,
                proposals,
            }) => {
                assert_eq!(
                    source.to_string_representation(),
                    "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
                );
                assert_eq!(*period, 719);
                assert_eq!(proposals.len(), 2);
                assert_eq!(
                    proposals[0].to_base58_check(),
                    "PscqRYywd243M2eZspXZEJGsRmNchp4ZKfKmoyEZTRHeLQvVGjp"
                );
                assert_eq!(
                    proposals[1].to_base58_check(),
                    "PscqRYywd243M2eZspXZEJGsRmNchp4ZKfKmoyEZTRHeLQvVGjp"
                );
            }
            _ => panic!("proposals expected"),
        }
    });

    operation_contents_test!(ballot, contents, {
        match &contents[0] {
            Contents::Ballot(BallotOperation {
                source,
                period,
                proposal,
                ballot,
            }) => {
                assert_eq!(
                    source.to_string_representation(),
                    "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
                );
                assert_eq!(*period, 719);
                assert_eq!(
                    proposal.to_base58_check(),
                    "PscqRYywd243M2eZspXZEJGsRmNchp4ZKfKmoyEZTRHeLQvVGjp"
                );
                assert!(matches!(*ballot, Ballot::Nay));
            }
            _ => panic!("ballot expected"),
        }
    });

    operation_contents_test!(reveal, contents, {
        match &contents[0] {
            Contents::Reveal(RevealOperation {
                source,
                fee,
                counter,
                gas_limit,
                storage_limit,
                public_key,
            }) => {
                assert_eq!(
                    source.to_string_representation(),
                    "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
                );
                assert_eq!(fee.0, BigInt::from(33));
                assert_eq!(counter.0, BigInt::from(732));
                assert_eq!(gas_limit.0, BigInt::from(9451117));
                assert_eq!(storage_limit.0, BigInt::from(57024931117_u64));
                assert_eq!(
                    public_key.to_string_representation(),
                    "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav"
                );
            }
            _ => panic!("reveal expected"),
        }
    });

    operation_contents_test!(transaction_to_implicit, contents, {
        match &contents[0] {
            Contents::Transaction(TransactionOperation {
                source,
                fee,
                counter,
                gas_limit,
                storage_limit,
                amount,
                destination,
                parameters,
            }) => {
                assert_eq!(
                    source.to_string_representation(),
                    "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
                );
                assert_eq!(fee.0, BigInt::from(33));
                assert_eq!(counter.0, BigInt::from(732));
                assert_eq!(gas_limit.0, BigInt::from(9451117));
                assert_eq!(storage_limit.0, BigInt::from(57024931117_u64));
                assert_eq!(amount.0, BigInt::from(407));
                match destination {
                    ContractId::Implicit(implicit) => assert_eq!(
                        implicit.to_string_representation(),
                        "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
                    ),
                    ContractId::Originated(_) => {
                        panic!("unexpected implicit destination")
                    }
                }
                assert!(parameters.is_none());
            }
            _ => panic!("transaction expected"),
        }
    });

    operation_contents_test!(transaction_to_originated, contents, {
        match &contents[0] {
            Contents::Transaction(TransactionOperation {
                source,
                fee,
                counter,
                gas_limit,
                storage_limit,
                amount,
                destination,
                parameters,
            }) => {
                assert_eq!(
                    source.to_string_representation(),
                    "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
                );
                assert_eq!(fee.0, BigInt::from(33));
                assert_eq!(counter.0, BigInt::from(732));
                assert_eq!(gas_limit.0, BigInt::from(9451117));
                assert_eq!(storage_limit.0, BigInt::from(57024931117_u64));
                assert_eq!(amount.0, BigInt::from(407));
                // TODO figure out how to store any contract hash
                match destination {
                    ContractId::Originated(_originated) => (),
                    ContractId::Implicit(_) => {
                        panic!("unexpected implicit destination")
                    }
                }
                assert!(parameters.is_some());
            }
            _ => panic!("transaction expected"),
        }
    });

    operation_contents_test!(origination, contents, {
        match &contents[0] {
            Contents::Origination(OriginationOperation {
                source,
                fee,
                counter,
                gas_limit,
                storage_limit,
                balance,
                delegate,
                script: _,
            }) => {
                assert_eq!(
                    source.to_string_representation(),
                    "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
                );
                assert_eq!(fee.0, BigInt::from(33));
                assert_eq!(counter.0, BigInt::from(732));
                assert_eq!(gas_limit.0, BigInt::from(9451117));
                assert_eq!(storage_limit.0, BigInt::from(57024931117_u64));
                assert_eq!(balance.0, BigInt::from(84143));
                assert_eq!(
                    delegate
                        .as_ref()
                        .map(SignaturePublicKeyHash::to_string_representation),
                    Some("tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx".to_string())
                );
            }
            _ => panic!("origination expected"),
        }
    });

    operation_contents_test!(delegation, contents, {
        match &contents[0] {
            Contents::Delegation(DelegationOperation {
                source,
                fee,
                counter,
                gas_limit,
                storage_limit,
                delegate,
            }) => {
                assert_eq!(
                    source.to_string_representation(),
                    "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
                );
                assert_eq!(fee.0, BigInt::from(33));
                assert_eq!(counter.0, BigInt::from(732));
                assert_eq!(gas_limit.0, BigInt::from(9451117));
                assert_eq!(storage_limit.0, BigInt::from(57024931117_u64));
                assert_eq!(
                    delegate
                        .as_ref()
                        .map(SignaturePublicKeyHash::to_string_representation),
                    Some("tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx".to_string())
                );
            }
            _ => panic!("delegation expected"),
        }
    });

    operation_contents_test!(delegation_withdrawal, contents, {
        match &contents[0] {
            Contents::Delegation(DelegationOperation {
                source,
                fee,
                counter,
                gas_limit,
                storage_limit,
                delegate,
            }) => {
                assert_eq!(
                    source.to_string_representation(),
                    "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
                );
                assert_eq!(fee.0, BigInt::from(33));
                assert_eq!(counter.0, BigInt::from(732));
                assert_eq!(gas_limit.0, BigInt::from(9451117));
                assert_eq!(storage_limit.0, BigInt::from(57024931117_u64));
                assert!(delegate.is_none());
            }
            _ => panic!("delegation expected"),
        }
    });

    /*
    operation_contents_test!(register_global_constant, contents, {
        match &contents[0] {
            Contents::RegisterGlobalConstant(RegisterGlobalConstantOperation { source, fee, counter, gas_limit, storage_limit, value }) => {
                assert_eq!(
                    source.to_string_representation(),
                    "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
                );
                assert_eq!(fee.0, BigInt::from(33));
                assert_eq!(counter.0, BigInt::from(732));
                assert_eq!(gas_limit.0, BigInt::from(9451117));
                assert_eq!(storage_limit.0, BigInt::from(57024931117_u64));
            }
            _ => panic!("register_global_constant expected"),
        }
    });
    */
}
