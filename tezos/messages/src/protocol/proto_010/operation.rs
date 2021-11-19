// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Operation contents. This is the contents of the opaque field [super::operation::Operation::data].
//! See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#alpha-operation-alpha-contents-determined-from-data-8-bit-tag].
//!
//! Comparing to the previous protocol, new field [`FullHeader::liquidity_baking_escape_vote`] has been added.
//! See https://tezos.gitlab.io/protocols/010_granada.html#metadata-changes.

pub use super::super::proto_009::operation::{
    ActivateAccountOperation, BallotOperation, ContractId, DelegationOperation,
    DoubleEndorsementEvidenceOperation, EndorsementOperation, EndorsementWithSlotOperation,
    FailingNoopOperation, InlinedEndorsement, InlinedEndorsementContents,
    InlinedEndorsementVariant, OriginationOperation, ProposalsOperation,
    RegisterGlobalConstantOperation, RevealOperation, SeedNonceRevelationOperation,
    TransactionOperation,
};

use std::convert::TryFrom;

use crypto::hash::{BlockHash, ContextHash, HashTrait, OperationListListHash, Signature};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_encoding::{encoding::HasEncoding, nom::NomReader};

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
///
/// Comparing to [super::super::proto_001::operation::Content], new variant [Operation::FailingNoop] is added.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader)]
pub struct DoubleBakingEvidenceOperation {
    #[encoding(dynamic)]
    pub bh1: FullHeader,
    #[encoding(dynamic)]
    pub bh2: FullHeader,
}

/// Full Header.
/// See [https://tezos.gitlab.io/shell/p2p_api.html?highlight=p2p%20encodings#endorsement-tag-0].
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
    pub priority: u16,
    #[encoding(sized = "8", bytes)]
    pub proof_of_work_nonce: Vec<u8>,
    #[encoding(option, sized = "32", bytes)]
    pub seed_nonce_hash: Option<Vec<u8>>,
    pub liquidity_baking_escape_vote: bool,
    pub signature: Signature,
}

#[cfg(test)]
mod tests {

    use std::{fs::File, io::Read, path::PathBuf};

    use anyhow::{Context, Result};
    use num_bigint::BigInt;

    use crate::base::signature_public_key::SignaturePublicKeyHash;
    use crate::p2p::binary_message::BinaryRead;
    use crate::p2p::encoding::block_header::display_fitness;
    use crate::p2p::encoding::operation::Operation as P2POperation;

    use super::*;
    use super::{Contents, Operation};

    const DATA_DIR_NAME: &str = "010_granada";

    fn read_data(file: &str) -> Result<Vec<u8>> {
        let dir = std::env::var("CARGO_MANIFEST_DIR")
            .with_context(|| format!("`CARGO_MANIFEST_DIR` is not set"))?;
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
	    ($name:ident, $branch:literal, $signature:expr, $contents:ident, $contents_assert:block) => {
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
                if $signature.is_some() {
                    assert_eq!(&signature.to_base58_check(), $signature.unwrap());
                }
                assert_eq!($contents.len(), 1);
                $contents_assert;

                let operation = P2POperation::from_bytes(&bytes)?;
                let Operation {
                    branch,
                    $contents,
                    signature,
                } = operation.clone().try_into()?;
                assert_eq!(
                    branch.to_base58_check(),
                    $branch
                );
                if $signature.is_some() {
                    assert_eq!(&signature.to_base58_check(), $signature.unwrap());
                }
                assert_eq!($contents.len(), 1);
                $contents_assert;

                let operation = P2POperation::from_bytes(&bytes)?;
                let OperationContents {
                    $contents,
                    signature,
                } = operation.clone().try_into()?;
                if $signature.is_some() {
                    assert_eq!(&signature.to_base58_check(), $signature.unwrap());
                }
                assert_eq!($contents.len(), 1);
                $contents_assert;

                Ok(())
            }
	    };
	    ($name:ident, $contents:ident, $contents_assert:block) => {
            operation_contents_test!($name, "BKpbfCvh777DQHnXjU2sqHvVUNZ7dBAdqEfKkdw8EGSkD9LSYXb", Some("sigbQ5ZNvkjvGssJgoAnUAfY4Wvvg3QZqawBYB1j1VDBNTMBAALnCzRHWzer34bnfmzgHg3EvwdzQKdxgSghB897cono6gbQ"), $contents, $contents_assert);
        };
    }

    operation_contents_test!(endorsement, contents, {
        match contents[0] {
            Contents::Endorsement(EndorsementOperation { level }) => assert_eq!(level, 1331),
            _ => assert!(false, "endorsement expected"),
        }
    });

    operation_contents_test!(seed_nonce_revelation, contents, {
        match &contents[0] {
            Contents::SeedNonceRevelation(SeedNonceRevelationOperation { level, nonce }) => {
                assert_eq!(*level, 1331);
                assert_eq!(nonce, &[0; 32]);
            }
            _ => assert!(false, "seed nonce revelation expected"),
        }
    });

    operation_contents_test!(double_endorsement_evidence, contents, {
        match &contents[0] {
            Contents::DoubleEndorsementEvidence(DoubleEndorsementEvidenceOperation {
                op1,
                op2,
                slot,
            }) => {
                assert_eq!(*slot, 0);
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
            _ => assert!(false, "double endorsement evidence expected"),
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
                assert_eq!(bh1.liquidity_baking_escape_vote, false);
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
                assert_eq!(bh2.liquidity_baking_escape_vote, false);
                assert_eq!(hex::encode(&bh2.proof_of_work_nonce), "101895ca00000000");
                assert_eq!(bh2.signature.to_base58_check(), "sigbQ5ZNvkjvGssJgoAnUAfY4Wvvg3QZqawBYB1j1VDBNTMBAALnCzRHWzer34bnfmzgHg3EvwdzQKdxgSghB897cono6gbQ");
            }
            _ => assert!(false, "double baking evidence expected"),
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
            _ => assert!(false, "activate account expected"),
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
            _ => assert!(false, "proposals expected"),
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
                assert_eq!(*ballot, 0);
            }
            _ => assert!(false, " expected"),
        }
    });

    operation_contents_test!(endorsement_with_slot, contents, {
        match &contents[0] {
            Contents::EndorsementWithSlot(EndorsementWithSlotOperation {
                endorsement:
                    InlinedEndorsement {
                        branch,
                        operations,
                        signature,
                    },
                slot,
            }) => {
                assert_eq!(*slot, 0);
                assert_eq!(
                    branch.to_base58_check(),
                    "BKpbfCvh777DQHnXjU2sqHvVUNZ7dBAdqEfKkdw8EGSkD9LSYXb"
                );
                assert!(matches!(
                    operations,
                    InlinedEndorsementContents::Endorsement(InlinedEndorsementVariant {
                        level: 1331
                    })
                ));
                assert_eq!(signature.to_base58_check(), "sigbQ5ZNvkjvGssJgoAnUAfY4Wvvg3QZqawBYB1j1VDBNTMBAALnCzRHWzer34bnfmzgHg3EvwdzQKdxgSghB897cono6gbQ");
            }
            _ => assert!(false, "endorsement with slot expected"),
        }
    });

    operation_contents_test!(
        endorsement_with_slot1,
        "BKjKKYPeqaQLmdsw34Fa2KF8scdCBggae1eWyEoaQnFj45vSQgX",
        Option::<&str>::None,
        contents,
        {
            match &contents[0] {
                Contents::EndorsementWithSlot(EndorsementWithSlotOperation {
                    endorsement:
                        InlinedEndorsement {
                            branch,
                            operations,
                            signature,
                        },
                    slot,
                }) => {
                    assert_eq!(*slot, 41);
                    assert_eq!(
                        branch.to_base58_check(),
                        "BKjKKYPeqaQLmdsw34Fa2KF8scdCBggae1eWyEoaQnFj45vSQgX"
                    );
                    assert!(matches!(
                        operations,
                        InlinedEndorsementContents::Endorsement(InlinedEndorsementVariant {
                            level: 700456
                        })
                    ));
                    assert_eq!(signature.to_base58_check(), "sigkFFGNQ1V2oFRqrxvw9KkzDzyxDcGTFu39hbNeAjHx52mXL4M3SdZZwY9xPQ1AsqHhb41k2x7ubLx7H7yRmtb8ANCa7324");
                }
                _ => assert!(false, "endorsement with slot expected"),
            }
        }
    );

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
            _ => assert!(false, "reveal expected"),
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
                        assert!(false, "unexpected implicit destination")
                    }
                }
                assert!(parameters.is_none());
            }
            _ => assert!(false, "transaction expected"),
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
                        assert!(false, "unexpected implicit destination")
                    }
                }
                assert!(parameters.is_some());
            }
            _ => assert!(false, "transaction expected"),
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
            _ => assert!(false, "origination expected"),
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
            _ => assert!(false, "delegation expected"),
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
            _ => assert!(false, "delegation expected"),
        }
    });

    /* TODO implement this test when data is available
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
            _ => assert!(false, "register_global_constant expected"),
        }
    });
    */
}
