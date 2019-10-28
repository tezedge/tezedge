// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::fmt;

use serde::{Deserialize, Serialize};

use tezos_encoding::hash::{BlockHash, ChainId, HashEncoding, HashType, ProtocolHash};
use tezos_interop::ffi;
use tezos_interop::ffi::{ApplyBlockError, ApplyBlockResult, BlockHeaderError, OcamlRuntimeConfiguration, OcamlRuntimeConfigurationError, OcamlStorageInitError, OcamlStorageInitInfo, TestChain};
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

use crate::environment;
use crate::environment::{TezosEnvironment, TezosEnvironmentConfiguration};

pub type TezosRuntimeConfiguration = OcamlRuntimeConfiguration;

pub fn change_runtime_configuration(settings: TezosRuntimeConfiguration) -> Result<(), OcamlRuntimeConfigurationError> {
    match ffi::change_runtime_configuration(settings) {
        Ok(result) => Ok(result?),
        Err(e) => {
            Err(OcamlRuntimeConfigurationError::ChangeConfigurationError {
                message: format!("FFI 'change_runtime_configuration' failed! Reason: {:?}", e)
            })
        }
    }
}

/// Struct represent init information about Tezos OCaml storage
#[derive(Serialize, Deserialize)]
pub struct TezosStorageInitInfo {
    pub chain_id: ChainId,
    pub test_chain: Option<TezosTestChain>,
    pub genesis_block_header_hash: BlockHash,
    pub genesis_block_header: BlockHeader,
    pub current_block_header_hash: BlockHash,
    pub supported_protocol_hashes: Vec<ProtocolHash>,
}

pub type TezosTestChain = TestChain;

impl TezosStorageInitInfo {
    fn new(storage_init_info: OcamlStorageInitInfo) -> Result<Self, OcamlStorageInitError> {
        let genesis_header = match BlockHeader::from_bytes(storage_init_info.genesis_block_header) {
            Ok(header) => header,
            Err(e) => return Err(OcamlStorageInitError::InitializeError { message: format!("Decoding from hex failed! Reason: {:?}", e) })
        };
        Ok(TezosStorageInitInfo {
            chain_id: storage_init_info.chain_id,
            test_chain: storage_init_info.test_chain,
            genesis_block_header_hash: storage_init_info.genesis_block_header_hash,
            genesis_block_header: genesis_header,
            current_block_header_hash: storage_init_info.current_block_header_hash,
            supported_protocol_hashes: storage_init_info.supported_protocol_hashes,
        })
    }
}

impl fmt::Debug for TezosStorageInitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let chain_hash_encoding = HashEncoding::new(HashType::ChainId);
        let block_hash_encoding = HashEncoding::new(HashType::BlockHash);
        write!(f, "TezosStorageInitInfo {{ chain_id: {}, genesis_block_header_hash: {}, current_block_header_hash: {} }}",
               chain_hash_encoding.bytes_to_string(&self.chain_id),
               block_hash_encoding.bytes_to_string(&self.genesis_block_header_hash),
               block_hash_encoding.bytes_to_string(&self.current_block_header_hash))
    }
}


/// Initializes storage for Tezos ocaml storage in chosen directory
pub fn init_storage(storage_data_dir: String, tezos_environment: TezosEnvironment) -> Result<TezosStorageInitInfo, OcamlStorageInitError> {
    let cfg: &TezosEnvironmentConfiguration = match environment::TEZOS_ENV.get(&tezos_environment) {
        None => return Err(OcamlStorageInitError::InitializeError {
            message: format!("FFI 'init_storage' failed, because there is no tezos environment configured for: {:?}", tezos_environment)
        }),
        Some(cfg) => cfg
    };
    match ffi::init_storage(storage_data_dir, &cfg.genesis) {
        Ok(result) => Ok(TezosStorageInitInfo::new(result?)?),
        Err(e) => {
            Err(OcamlStorageInitError::InitializeError {
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
                Err(_) => Err(BlockHeaderError::ReadError { message: "Decoding from hex failed!".to_string() })
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
    block_header_hash: &BlockHash,
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
        block_header_hash.clone(),
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