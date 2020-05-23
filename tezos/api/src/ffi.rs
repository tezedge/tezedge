// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

/// Rust implementation of messages required for Rust <-> OCaml FFI communication.

use derive_builder::Builder;
use failure::Fail;
use serde::{Deserialize, Serialize};

use crypto::hash::{ChainId, ContextHash, HashType};
use lazy_static::lazy_static;
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_messages::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation, OperationsForBlocksMessage};

lazy_static! {
    pub static ref APPLY_BLOCK_REQUEST_ENCODING: Encoding = Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashType::ChainId)),
            Field::new("block_header", Encoding::dynamic(BlockHeader::encoding())),
            Field::new("pred_header", Encoding::dynamic(BlockHeader::encoding())),
            Field::new("max_operations_ttl", Encoding::Int31),
            Field::new("operations", Encoding::dynamic(Encoding::list(Encoding::dynamic(Encoding::list(Encoding::dynamic(Operation::encoding())))))),
    ]);
}

pub type RustBytes = Vec<u8>;

/// Genesis block information structure
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenesisChain {
    pub time: String,
    pub block: String,
    pub protocol: String,
}

/// Voted protocol overrides
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProtocolOverrides {
    pub forced_protocol_upgrades: Vec<(i32, String)>,
    pub voted_protocol_overrides: Vec<(String, String)>,
}

/// Test chain information
#[derive(Debug, Serialize, Deserialize)]
pub struct TestChain {
    pub chain_id: RustBytes,
    pub protocol_hash: RustBytes,
    pub expiration_date: String,
}

/// Holds configuration for ocaml runtime - e.g. arguments which are passed to ocaml and can be change in runtime
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TezosRuntimeConfiguration {
    pub log_enabled: bool,
    pub no_of_ffi_calls_treshold_for_gc: i32,
    pub debug_mode: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Builder)]
pub struct ApplyBlockRequest {
    pub chain_id: ChainId,
    pub block_header: BlockHeader,
    pub pred_header: BlockHeader,
    pub max_operations_ttl: i32,
    pub operations: Vec<Vec<Operation>>,
    #[serde(skip_serializing)]
    #[builder(default)]
    body: BinaryDataCache,
}

impl ApplyBlockRequest {
    pub fn to_ops(block_operations: &Vec<Option<OperationsForBlocksMessage>>) -> Vec<Vec<Operation>> {
        let mut operations = Vec::with_capacity(block_operations.len());

        for block_ops in block_operations {
            if let Some(bo_ops) = block_ops {
                operations.push(bo_ops.operations().clone());
            } else {
                operations.push(vec![]);
            }
        }

        operations
    }
}

impl CachedData for ApplyBlockRequest {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

/// Application block result
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ApplyBlockResult {
    pub validation_result_message: String,
    pub context_hash: RustBytes,
    pub block_header_proto_json: String,
    pub block_header_proto_metadata_json: String,
    pub operations_proto_metadata_json: String,
    pub max_operations_ttl: u16,
    pub last_allowed_fork_level: i32,
    pub forking_testchain: bool,
    pub forking_testchain_data: Option<ForkingTestchainData>,
}

/// Init protocol context result
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct InitProtocolContextResult {
    pub supported_protocol_hashes: Vec<RustBytes>,
    /// Presents only if was genesis commited to context
    pub genesis_commit_hash: Option<ContextHash>,
}

/// Commit genesis result
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct CommitGenesisResult {
    pub block_header_proto_json: String,
    pub block_header_proto_metadata_json: String,
    pub operations_proto_metadata_json: String,
}

/// Forking test chain data
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ForkingTestchainData {
    pub genesis: RustBytes,
    pub chain_id: RustBytes,
}

#[derive(Serialize, Deserialize, Debug, Fail)]
pub enum TezosRuntimeConfigurationError {
    #[fail(display = "Change ocaml settings failed, message: {}!", message)]
    ChangeConfigurationError {
        message: String
    }
}

impl From<ocaml::Error> for TezosRuntimeConfigurationError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            ocaml::Error::Exception(ffi_error) => {
                TezosRuntimeConfigurationError::ChangeConfigurationError {
                    message: parse_error_message(ffi_error).unwrap_or_else(|| "unknown".to_string())
                }
            }
            _ => panic!("Ocaml settings failed! Reason: {:?}", error)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Fail)]
pub enum TezosGenerateIdentityError {
    #[fail(display = "Generate identity failed, message: {}!", message)]
    GenerationError {
        message: String
    },
    #[fail(display = "Generated identity is invalid json! message: {}!", message)]
    InvalidJsonError {
        message: String
    },
}

impl From<ocaml::Error> for TezosGenerateIdentityError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            ocaml::Error::Exception(ffi_error) => {
                TezosGenerateIdentityError::GenerationError {
                    message: parse_error_message(ffi_error).unwrap_or_else(|| "unknown".to_string())
                }
            }
            _ => panic!("Generate identity failed! Reason: {:?}", error)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Fail)]
pub enum TezosStorageInitError {
    #[fail(display = "Ocaml storage init failed, message: {}!", message)]
    InitializeError {
        message: String
    }
}

impl From<ocaml::Error> for TezosStorageInitError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            ocaml::Error::Exception(ffi_error) => {
                TezosStorageInitError::InitializeError {
                    message: parse_error_message(ffi_error).unwrap_or_else(|| "unknown".to_string())
                }
            }
            _ => panic!("Storage initialization failed! Reason: {:?}", error)
        }
    }
}

impl slog::Value for TezosStorageInitError {
    fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

#[derive(Serialize, Deserialize, Debug, Fail)]
pub enum GetDataError {
    #[fail(display = "Ocaml failed to get data, message: {}!", message)]
    ReadError {
        message: String
    }
}

impl From<ocaml::Error> for GetDataError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            ocaml::Error::Exception(ffi_error) => {
                GetDataError::ReadError {
                    message: parse_error_message(ffi_error).unwrap_or_else(|| "unknown".to_string())
                }
            }
            _ => panic!("Get data failed! Reason: {:?}", error)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Fail, PartialEq)]
pub enum ApplyBlockError {
    #[fail(display = "Incomplete operations, exptected: {}, has actual: {}!", expected, actual)]
    IncompleteOperations {
        expected: usize,
        actual: usize,
    },
    #[fail(display = "Failed to apply block - message: {}!", message)]
    FailedToApplyBlock {
        message: String,
    },
    #[fail(display = "Unknown predecessor context - try to apply predecessor at first message: {}!", message)]
    UnknownPredecessorContext {
        message: String,
    },
    #[fail(display = "Predecessor does not match - message: {}!", message)]
    PredecessorMismatch {
        message: String,
    },
    #[fail(display = "Invalid apply block request data - message: {}!", message)]
    InvalidApplyBlockRequestData {
        message: String,
    },
}

impl From<ocaml::Error> for ApplyBlockError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            ocaml::Error::Exception(ffi_error) => {
                match parse_error_message(ffi_error) {
                    None => ApplyBlockError::FailedToApplyBlock {
                        message: "unknown".to_string()
                    },
                    Some(message) => {
                        match message.as_str() {
                            e if e.starts_with("Unknown_predecessor_context") => ApplyBlockError::UnknownPredecessorContext {
                                message: message.to_string()
                            },
                            e if e.starts_with("Predecessor_mismatch") => ApplyBlockError::PredecessorMismatch {
                                message: message.to_string()
                            },
                            message => ApplyBlockError::FailedToApplyBlock {
                                message: message.to_string()
                            }
                        }
                    }
                }
            }
            _ => panic!("Unhandled ocaml error occurred for apply block! Error: {:?}", error)
        }
    }
}

#[derive(Debug, Fail)]
pub enum BlockHeaderError {
    #[fail(display = "BlockHeader cannot be read from storage: {}!", message)]
    ReadError {
        message: String
    },
    #[fail(display = "BlockHeader was expected, but was not found!")]
    ExpectedButNotFound,
}

impl From<ocaml::Error> for BlockHeaderError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            ocaml::Error::Exception(ffi_error) => {
                BlockHeaderError::ReadError {
                    message: parse_error_message(ffi_error).unwrap_or_else(|| "unknown".to_string())
                }
            }
            _ => panic!("Storage initialization failed! Reason: {:?}", error)
        }
    }
}

#[derive(Debug, Fail)]
pub enum ContextDataError {
    #[fail(display = "Resolve/decode context data failed to decode: {}!", message)]
    DecodeError {
        message: String
    },
}

impl From<ocaml::Error> for ContextDataError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            ocaml::Error::Exception(ffi_error) => {
                ContextDataError::DecodeError {
                    message: parse_error_message(ffi_error).unwrap_or_else(|| "unknown".to_string())
                }
            }
            _ => panic!("Resolve context data failed! Reason: {:?}", error)
        }
    }
}

fn parse_error_message(ffi_error: ocaml::Value) -> Option<String> {
    if ffi_error.is_block() {
        // for exceptions, in the field 2, there is a message for Failure or Ffi_error
        let error_message = ffi_error.field(1);
        if error_message.tag() == ocaml::Tag::String {
            let error_message: ocaml::Str = error_message.into();
            return Some(error_message.as_str().to_string());
        }
    }
    None
}
