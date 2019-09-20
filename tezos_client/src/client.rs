use log::error;

use networking::p2p::binary_message::Hexable;
use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{BlockHash, ChainId};
use tezos_interop::ffi;
use tezos_interop::ffi::{ApplyBlockError, ApplyBlockResult, BlockHeaderError, OcamlStorageInitError, OcamlStorageInitInfo};

/// Struct represent init information about Tezos OCaml storage
pub struct TezosStorageInitInfo {
    pub chain_id: ChainId,
    pub genesis_block_header_hash: BlockHash,
    pub current_block_header_hash: BlockHash,
}

impl TezosStorageInitInfo {
    fn new(storage_init_info: OcamlStorageInitInfo) -> Self {
        TezosStorageInitInfo {
            chain_id: hex::decode(storage_init_info.chain_id).unwrap(),
            genesis_block_header_hash: hex::decode(storage_init_info.genesis_block_header_hash).unwrap(),
            current_block_header_hash: hex::decode(storage_init_info.current_block_header_hash).unwrap(),
        }
    }
}

/// Initializes storage for Tezos ocaml storage in chosen directory
pub fn init_storage(storage_data_dir: String) -> Result<TezosStorageInitInfo, OcamlStorageInitError> {
    match ffi::init_storage(storage_data_dir) {
        Ok(result) => Ok(TezosStorageInitInfo::new(result?)),
        Err(e) => {
            error!("Init ocaml storage failed! Reason: {:?}", e);
            Err(OcamlStorageInitError::InitializeError {
                message: "Ffi 'init_storage' failed! Initialization of Tezos storage failed, this storage is required, we can do nothing without that!".to_string()
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
                message: "Ffi 'init_storage' failed! Initialization of Tezos storage failed, this storage is required, we can do nothing without that!".to_string()
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
                        Err(_) => Err(BlockHeaderError::ReadError { message: "Decoding from hex failed!".to_string() })
                    }
                }
            }
        },
        Err(e) => {
            error!("Get block header failed! Reason: {:?}", e);
            Err(BlockHeaderError::ReadError {
                message: "Ffi 'get_block_header' failed! Something is wrong!".to_string()
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
    if (block_header.validation_pass as usize) != operations.len() {
        return Err(ApplyBlockError::IncompleteOperations {
            expected: block_header.validation_pass as usize,
            actual: operations.len(),
        });
    }

    let block_header = block_header.to_hex();
    if let Err(_) = block_header {
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
        .into_iter()
        .map(|bo| {
            if let Some(bo_ops) = bo {
                Some(
                    bo_ops.operations
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