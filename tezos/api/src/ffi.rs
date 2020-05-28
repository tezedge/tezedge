// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt::Debug;

/// Rust implementation of messages required for Rust <-> OCaml FFI communication.

use derive_builder::Builder;
use failure::Fail;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;

use crypto::hash::{BlockHash, ChainId, ContextHash, HashType};
use lazy_static::lazy_static;
use tezos_encoding::{binary_writer, ser};
use tezos_encoding::binary_reader::{BinaryReader, BinaryReaderError};
use tezos_encoding::de::from_value as deserialize_from_value;
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation, OperationsForBlocksMessage};

pub type RustBytes = Vec<u8>;

/// Trait for binary encoding messages for ffi.
pub trait FfiMessage: DeserializeOwned + Serialize + Sized + Send + PartialEq + Debug {

    #[inline]
    fn as_rust_bytes(&self) -> Result<RustBytes, ser::Error> {
        binary_writer::write(&self, Self::encoding())
    }

    /// Create new struct from bytes.
    #[inline]
    fn from_rust_bytes(buf: RustBytes) -> Result<Self, BinaryReaderError> {
        let value = BinaryReader::new().read(buf, Self::encoding())?;
        let value: Self = deserialize_from_value(&value)?;
        Ok(value)
    }

    fn encoding() -> &'static Encoding;
}

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

#[derive(Serialize, Deserialize, Debug, Clone, Builder, PartialEq)]
pub struct ApplyBlockRequest {
    pub chain_id: ChainId,
    pub block_header: BlockHeader,
    pub pred_header: BlockHeader,
    pub max_operations_ttl: i32,
    pub operations: Vec<Vec<Operation>>,
}

impl ApplyBlockRequest {
    pub fn convert_operations(block_operations: &Vec<Option<OperationsForBlocksMessage>>) -> Vec<Vec<Operation>> {
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

lazy_static! {
    pub static ref APPLY_BLOCK_REQUEST_ENCODING: Encoding = Encoding::Obj(vec![
        Field::new("chain_id", Encoding::Hash(HashType::ChainId)),
        Field::new("block_header", Encoding::dynamic(BlockHeader::encoding())),
        Field::new("pred_header", Encoding::dynamic(BlockHeader::encoding())),
        Field::new("max_operations_ttl", Encoding::Int31),
        Field::new("operations", Encoding::dynamic(Encoding::list(Encoding::dynamic(Encoding::list(Encoding::dynamic(Operation::encoding())))))),
    ]);
}

impl FfiMessage for ApplyBlockRequest {
    fn encoding() -> &'static Encoding {
        &APPLY_BLOCK_REQUEST_ENCODING
    }
}

/// Application block result
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ApplyBlockResponse {
    pub validation_result_message: String,
    pub context_hash: ContextHash,
    pub block_header_proto_json: String,
    pub block_header_proto_metadata_json: String,
    pub operations_proto_metadata_json: String,
    pub max_operations_ttl: i32,
    pub last_allowed_fork_level: i32,
    pub forking_testchain: bool,
    pub forking_testchain_data: Option<ForkingTestchainData>,
}

lazy_static! {
    pub static ref FORKING_TESTCHAIN_DATA_ENCODING: Encoding = Encoding::Obj(vec![
        Field::new("genesis", Encoding::Hash(HashType::BlockHash)),
        Field::new("chain_id", Encoding::Hash(HashType::ChainId)),
    ]);

    pub static ref APPLY_BLOCK_RESPONSE_ENCODING: Encoding = Encoding::Obj(vec![
        Field::new("validation_result_message", Encoding::String),
        Field::new("context_hash", Encoding::Hash(HashType::ContextHash)),
        Field::new("block_header_proto_json", Encoding::String),
        Field::new("block_header_proto_metadata_json", Encoding::String),
        Field::new("operations_proto_metadata_json", Encoding::String),
        Field::new("max_operations_ttl", Encoding::Int31),
        Field::new("last_allowed_fork_level", Encoding::Int32),
        Field::new("forking_testchain", Encoding::Bool),
        Field::new("forking_testchain_data", Encoding::option(FORKING_TESTCHAIN_DATA_ENCODING.clone())),
    ]);
}

impl FfiMessage for ApplyBlockResponse {
    fn encoding() -> &'static Encoding {
        &APPLY_BLOCK_RESPONSE_ENCODING
    }
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
    pub genesis: BlockHash,
    pub chain_id: ChainId,
}

#[derive(Serialize, Deserialize, Debug, Fail, PartialEq)]
pub enum CallError {
    #[fail(display = "Failed to call - message: {:?}!", parsed_error_message)]
    FailedToCall {
        parsed_error_message: Option<String>,
    },
    #[fail(display = "Invalid request data - message: {}!", message)]
    InvalidRequestData {
        message: String,
    },
    #[fail(display = "Invalid response data - message: {}!", message)]
    InvalidResponseData {
        message: String,
    },
}

impl From<ocaml::Error> for CallError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            ocaml::Error::Exception(ffi_error) => {
                match parse_error_message(ffi_error) {
                    None => CallError::FailedToCall {
                        parsed_error_message: None
                    },
                    Some(message) => {
                        CallError::FailedToCall {
                            parsed_error_message: Some(message)
                        }
                    }
                }
            }
            _ => panic!("Unhandled ocaml error occurred for apply block! Error: {:?}", error)
        }
    }
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
    #[fail(display = "Invalid request/response data - message: {}!", message)]
    InvalidRequestResponseData {
        message: String,
    },
}

impl From<CallError> for ApplyBlockError {
    fn from(error: CallError) -> Self {
        match error {
            CallError::FailedToCall { parsed_error_message } => {
                match parsed_error_message {
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
            CallError::InvalidRequestData { message } => ApplyBlockError::InvalidRequestResponseData {
                message
            },
            CallError::InvalidResponseData { message } => ApplyBlockError::InvalidRequestResponseData {
                message
            },
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
