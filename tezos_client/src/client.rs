// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::fmt;

use log::error;

use networking::p2p::binary_message::Hexable;
use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{BlockHash, ChainId, HashEncoding, HashType};
use tezos_interop::ffi;
use tezos_interop::ffi::{ApplyBlockError, ApplyBlockResult, BlockHeaderError, OcamlRuntimeConfiguration, OcamlRuntimeConfigurationError, OcamlStorageInitError, OcamlStorageInitInfo};

pub type TezosRuntimeConfiguration = OcamlRuntimeConfiguration;

pub fn change_runtime_configuration(settings: TezosRuntimeConfiguration) -> Result<(), OcamlRuntimeConfigurationError> {
    match ffi::change_runtime_configuration(settings) {
        Ok(result) => Ok(result?),
        Err(e) => {
            error!("Change runtime configuration failed! Reason: {:?}", e);
            Err(OcamlRuntimeConfigurationError::ChangeConfigurationError {
                message: format!("FFI 'change_runtime_configuration' failed! Reason: {:?}", e)
            })
        }
    }
}

/// Struct represent init information about Tezos OCaml storage
pub struct TezosStorageInitInfo {
    pub chain_id: ChainId,
    pub genesis_block_header_hash: BlockHash,
    pub genesis_block_header: BlockHeader,
    pub current_block_header_hash: BlockHash,
}

impl TezosStorageInitInfo {
    fn new(storage_init_info: OcamlStorageInitInfo) -> Result<Self, OcamlStorageInitError> {
        let genesis_header = match BlockHeader::from_hex(storage_init_info.genesis_block_header) {
            Ok(header) => header,
            Err(e) => return Err(OcamlStorageInitError::InitializeError { message: format!("Decoding from hex failed! Reason: {:?}", e) })
        };
        Ok(TezosStorageInitInfo {
            chain_id: hex::decode(storage_init_info.chain_id).unwrap(),
            genesis_block_header_hash: hex::decode(storage_init_info.genesis_block_header_hash).unwrap(),
            genesis_block_header: genesis_header,
            current_block_header_hash: hex::decode(storage_init_info.current_block_header_hash).unwrap(),
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
pub fn init_storage(storage_data_dir: String) -> Result<TezosStorageInitInfo, OcamlStorageInitError> {
    match ffi::init_storage(storage_data_dir) {
        Ok(result) => Ok(TezosStorageInitInfo::new(result?)?),
        Err(e) => {
            error!("Init ocaml storage failed! Reason: {:?}", e);
            Err(OcamlStorageInitError::InitializeError {
                message: format!("FFI 'init_storage' failed! Initialization of Tezos storage failed, this storage is required, we can do nothing without that! Reason: {:?}", e)
            })
        }
    }
}

/// Get current header block from storage
pub fn get_current_block_header(chain_id: &ChainId) -> Result<BlockHeader, BlockHeaderError> {
    match ffi::get_current_block_header(hex::encode(chain_id)) {
        Ok(result) => {
            match BlockHeader::from_hex(result?) {
                Ok(header) => Ok(header),
                Err(_) => Err(BlockHeaderError::ReadError { message: "Decoding from hex failed!".to_string() })
            }
        },
        Err(e) => {
            error!("Get current header failed! Reason: {:?}", e);
            Err(BlockHeaderError::ReadError {
                message: format!("FFI 'get_current_block_header' failed! Initialization of Tezos storage failed, this storage is required, we can do nothing without that! Reason: {:?}", e)
            })
        }
    }
}

/// Get block header from storage or None
pub fn get_block_header(block_header_hash: &BlockHash) -> Result<Option<BlockHeader>, BlockHeaderError> {
    match ffi::get_block_header(hex::encode(block_header_hash)) {
        Ok(result) => {
            let header = result?;
            match header {
                None => Ok(None),
                Some(header) => {
                    match BlockHeader::from_hex(header) {
                        Ok(header) => Ok(Some(header)),
                        Err(e) => Err(BlockHeaderError::ReadError { message: format!("Decoding from hex failed! Reason: {:?}", e) })
                    }
                }
            }
        },
        Err(e) => {
            error!("Get block header failed! Reason: {:?}", e);
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
    block_header_hash: &BlockHash,
    block_header: &BlockHeader,
    operations: &Vec<Option<OperationsForBlocksMessage>>) -> Result<ApplyBlockResult, ApplyBlockError> {
    if (block_header.validation_pass() as usize) != operations.len() {
        return Err(ApplyBlockError::IncompleteOperations {
            expected: block_header.validation_pass() as usize,
            actual: operations.len(),
        });
    }

    let block_header = block_header.to_hex();
    if block_header.is_err() {
        return Err(ApplyBlockError::InvalidBlockHeaderData);
    }
    let block_header = block_header.unwrap();
    let operations = to_hex_vec(operations);

    match ffi::apply_block(
        hex::encode(block_header_hash),
        block_header,
        operations,
    ) {
        Ok(result) => result,
        Err(e) => {
            error!("Apply block failed! Reason: {:?}", e);
            Err(ApplyBlockError::FailedToApplyBlock {
                message: "Unknown OcamlError".to_string()
            })
        }
    }
}

fn to_hex_vec(block_operations: &Vec<Option<OperationsForBlocksMessage>>) -> Vec<Option<Vec<String>>> {
    block_operations
        .iter()
        .map(|bo| {
            if let Some(bo_ops) = bo {
                Some(
                    bo_ops.operations()
                        .iter()
                        .map(|op| op.to_hex().unwrap())
                        .collect()
                )
            } else {
                None
            }
        })
        .collect()
}