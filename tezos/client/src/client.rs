// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ChainId, ProtocolHash};
use tezos_api::client::TezosStorageInitInfo;
use tezos_api::environment::{self, TezosEnvironment, TezosEnvironmentConfiguration};
use tezos_api::ffi::{ApplyBlockError, ApplyBlockResult, BlockHeaderError, ContextDataError, TezosGenerateIdentityError, TezosRuntimeConfiguration, TezosRuntimeConfigurationError, TezosStorageInitError};
use tezos_api::identity::Identity;
use tezos_interop::ffi;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

/// Override runtime configuration for OCaml runtime
pub fn change_runtime_configuration(settings: TezosRuntimeConfiguration) -> Result<(), TezosRuntimeConfigurationError> {
    match ffi::change_runtime_configuration(settings) {
        Ok(result) => Ok(result?),
        Err(e) => {
            Err(TezosRuntimeConfigurationError::ChangeConfigurationError {
                message: format!("FFI 'change_runtime_configuration' failed! Reason: {:?}", e)
            })
        }
    }
}

/// Initializes storage for Tezos ocaml storage in chosen directory
pub fn init_storage(storage_data_dir: String, tezos_environment: TezosEnvironment, enable_testchain: bool) -> Result<TezosStorageInitInfo, TezosStorageInitError> {
    let cfg: &TezosEnvironmentConfiguration = match environment::TEZOS_ENV.get(&tezos_environment) {
        None => return Err(TezosStorageInitError::InitializeError {
            message: format!("FFI 'init_storage' failed, because there is no tezos environment configured for: {:?}", tezos_environment)
        }),
        Some(cfg) => cfg
    };
    match ffi::init_storage(storage_data_dir, &cfg.genesis, &cfg.protocol_overrides, enable_testchain) {
        Ok(result) => Ok(TezosStorageInitInfo::new(result?)
            .map_err(|err| TezosStorageInitError::InitializeError { message: format!("Decoding from hex failed! Reason: {:?}", err) })?),
        Err(e) => {
            Err(TezosStorageInitError::InitializeError {
                message: format!("FFI 'init_storage' failed! Initialization of Tezos storage failed, this storage is required, we can do nothing without that! Reason: {:?}", e)
            })
        }
    }
}

/// Get current header block from storage
pub fn get_current_block_header(chain_id: &ChainId) -> Result<BlockHeader, BlockHeaderError> {
    match ffi::get_current_block_header(chain_id.clone()) {
        Ok(result) => {
            match BlockHeader::from_bytes(result?) {
                Ok(header) => Ok(header),
                Err(_) => Err(BlockHeaderError::ReadError { message: "Decoding from bytes failed!".to_string() })
            }
        }
        Err(e) => {
            Err(BlockHeaderError::ReadError {
                message: format!("FFI 'get_current_block_header' failed! Initialization of Tezos storage failed, this storage is required, we can do nothing without that! Reason: {:?}", e)
            })
        }
    }
}

/// Get block header from storage or None
pub fn get_block_header(chain_id: &ChainId, block_header_hash: &BlockHash) -> Result<Option<BlockHeader>, BlockHeaderError> {
    match ffi::get_block_header(chain_id.clone(), block_header_hash.clone()) {
        Ok(result) => {
            let header = result?;
            match header {
                None => Ok(None),
                Some(header) => {
                    match BlockHeader::from_bytes(header) {
                        Ok(header) => Ok(Some(header)),
                        Err(e) => Err(BlockHeaderError::ReadError { message: format!("Decoding from hex failed! Reason: {:?}", e) })
                    }
                }
            }
        }
        Err(e) => {
            Err(BlockHeaderError::ReadError {
                message: format!("FFI 'get_block_header' failed! Something is wrong! Reason: {:?}", e)
            })
        }
    }
}

/// Applies new block to Tezos ocaml storage, means:
/// - block and operations are decoded by the protocol
/// - block and operations data are correctly stored in Tezos chain/storage
/// - new current head is evaluated
/// - returns validation_result.message
pub fn apply_block(
    chain_id: &ChainId,
    block_header: &BlockHeader,
    operations: &Vec<Option<OperationsForBlocksMessage>>) -> Result<ApplyBlockResult, ApplyBlockError> {

    // check operations count by validation_pass
    if (block_header.validation_pass() as usize) != operations.len() {
        return Err(ApplyBlockError::IncompleteOperations {
            expected: block_header.validation_pass() as usize,
            actual: operations.len(),
        });
    }

    let block_header = match block_header.as_bytes() {
        Err(e) => return Err(
            ApplyBlockError::InvalidBlockHeaderData {
                message: format!("Block header as_bytes failed: {:?}, block: {:?}", e, block_header)
            }
        ),
        Ok(data) => data
    };
    let operations = to_bytes(operations)?;

    match ffi::apply_block(
        chain_id.clone(),
        block_header,
        operations,
    ) {
        Ok(result) => result,
        Err(e) => {
            Err(ApplyBlockError::FailedToApplyBlock {
                message: format!("Unknown OcamlError: {:?}", e)
            })
        }
    }
}

/// Generate tezos identity
pub fn generate_identity(expected_pow: f64) -> Result<Identity, TezosGenerateIdentityError> {
    match ffi::generate_identity(expected_pow) {
        Ok(result) => Ok(result?),
        Err(e) => {
            Err(TezosGenerateIdentityError::GenerationError {
                message: format!("FFI 'generate_identity' failed! Reason: {:?}", e)
            })
        }
    }
}

/// Decode protocoled context data
pub fn decode_context_data(protocol_hash: ProtocolHash, key: Vec<String>, data: Vec<u8>) -> Result<Option<String>, ContextDataError> {
    match ffi::decode_context_data(protocol_hash, key, data) {
        Ok(result) => Ok(result?),
        Err(e) => {
            Err(ContextDataError::DecodeError {
                message: format!("FFI 'decode_context_data' failed! Reason: {:?}", e)
            })
        }
    }
}

fn to_bytes(block_operations: &Vec<Option<OperationsForBlocksMessage>>) -> Result<Vec<Option<Vec<Vec<u8>>>>, ApplyBlockError> {
    let mut operations = Vec::with_capacity(block_operations.len());

    for block_ops in block_operations {
        if let Some(bo_ops) = block_ops {
            let mut operations_by_pass = Vec::new();
            for bop in bo_ops.operations() {
                let op: Vec<u8> = match bop.as_bytes() {
                    Err(e) => return Err(
                        ApplyBlockError::InvalidOperationsData {
                            message: format!("Operation as_bytes failed: {:?}, operation: {:?}", e, bop)
                        }
                    ),
                    Ok(op_bytes) => op_bytes
                };
                operations_by_pass.push(op);
            }
            operations.push(Some(operations_by_pass));
        } else {
            operations.push(None);
        }
    }

    Ok(operations)
}