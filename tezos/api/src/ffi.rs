// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

/// Rust implementation of messages required for Rust <-> OCaml FFI communication.

use derive_new::new;
use failure::Fail;
use serde::{Deserialize, Serialize};

pub type RustBytes = Vec<u8>;

/// Genesis block information structure
#[derive(Debug)]
pub struct GenesisChain {
    pub time: String,
    pub block: String,
    pub protocol: String,
}

/// Voted protocol overrides
#[derive(Debug)]
pub struct ProtocolOverrides {
    pub forced_protocol_upgrades: Vec<(i32, String)>,
    pub voted_protocol_overrides: Vec<(String, String)>,
}

/// Storage initialization information for OCaml storage
#[derive(Debug)]
pub struct OcamlStorageInitInfo {
    pub chain_id: RustBytes,
    pub test_chain: Option<TestChain>,
    pub genesis_block_header_hash: RustBytes,
    pub genesis_block_header: RustBytes,
    pub current_block_header_hash: RustBytes,
    pub supported_protocol_hashes: Vec<RustBytes>,
}

/// Test chain information
#[derive(Debug, new, Serialize, Deserialize)]
pub struct TestChain {
    pub chain_id: RustBytes,
    pub protocol_hash: RustBytes,
    pub expiration_date: String,
}

/// Holds configuration for ocaml runtime - e.g. arguments which are passed to ocaml and can be change in runtime
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TezosRuntimeConfiguration {
    pub log_enabled: bool
}

/// Application block result
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ApplyBlockResult {
    pub validation_result_message: String,
    pub context_hash: RustBytes,
    pub block_header_proto_json: String,
    pub block_header_proto_metadata_json: String,
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
    #[fail(display = "Unknown predecessor - try to fetch predecessor at first!")]
    UnknownPredecessor,
    #[fail(display = "Invalid block header data - message: {}!", message)]
    InvalidBlockHeaderData {
        message: String,
    },
    #[fail(display = "Invalid operations data - message: {}!", message)]
    InvalidOperationsData {
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
                            "UnknownPredecessor" => ApplyBlockError::UnknownPredecessor,
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